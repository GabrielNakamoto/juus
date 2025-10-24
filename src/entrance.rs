use std::sync::Arc;
use crate::{Delivery, TransportStream, types::*};
use tokio::sync::{mpsc, Mutex, Notify};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
use mls_rs_crypto_openssl::OpensslCryptoProvider;
use mls_rs::{
	time::MlsTime,
	MlsMessage,
	MlsMessageDescription,
    Client,
	Group,
	group::ReceivedMessage,
	group::ReceivedMessage::*,
	client_builder::{MlsConfig, BaseConfig, WithIdentityProvider, WithCryptoProvider},
    identity::{SigningIdentity, basic::{BasicIdentityProvider, BasicCredential}},
    CipherSuite,
	ExtensionList
};
use sha3::{Digest, Sha3_256};


fn hash_topic(key: String) -> [u8; 32] {
	let topic = key + "_juus";
	let mut hasher = Sha3_256::new();
	hasher.update(topic.as_bytes());
	hasher.finalize().into()
}

fn gen_topic() -> [u8; 32] {
	use rand::RngCore;

	let mut rng = rand::rng();
	let mut bytes: [u8; 64] = [0; 64];
	rng.fill_bytes(&mut bytes);
	let mut hasher = Sha3_256::new();
	hasher.update(bytes);
	hasher.finalize().into()
}

type MyMlsConfig = WithIdentityProvider<
	BasicIdentityProvider,
	WithCryptoProvider<OpensslCryptoProvider, BaseConfig>
	>;

/*
** Use async channels


> Is a State struct necessary here?
  Do we need to query information or
  maybe handle shutdown? Otherwise
  this could just be 2 functions in a
  namespace

Application specific I/O interface
	* You give it Message structs, it handles encryption and delivery
	* It receives message structs, decrypts them and hands them to application

Initialization:
	* allocates delivery struct
	* establishes group status

2 main methods:
	open - Creates a new group and waits for active peers before
		   allowing application I/O.

		   Additionally listens for key
		   package messages (group join requests) => send app
		   notification over channel in response with an accept
		   function that the application can call to welcome the
		   new user.

	join - Sends key package to directory and waits for welcome
		   message before allowing application I/O
*/

type AsyncGroup = Arc<Mutex<Group<MyMlsConfig>>>;
use std::pin::Pin;

pub struct Entrance {
	mls: Client<MyMlsConfig>,
	delivery: Delivery,
	dirhash: Hash,
	nids: Option<Vec<iroh::NodeId>>
}

impl Entrance {
	fn build_e2ee_identity(name: &String) -> Client<MyMlsConfig> {
		use rand_core::OsRng;
		let mut crng = OsRng;
		let signer = SigningKey::generate(&mut crng);
		let verifier = signer.verifying_key();
		let secret = signer.to_bytes().to_vec().into();
		let public = verifier.to_bytes().to_vec().into();

		let identity = BasicCredential::new(name.as_bytes().to_vec());
		let s_identity = SigningIdentity::new(identity.into_credential(), public);

		mls_rs::Client::builder()
			.crypto_provider(OpensslCryptoProvider::default())
			.identity_provider(BasicIdentityProvider::new())
			.signing_identity(s_identity, secret, CipherSuite::CURVE25519_AES128)
			.build()
	}

	pub async fn new(
		name: &String,
		groupid: &String,
		nids: Option<Vec<iroh::NodeId>>
	) -> anyhow::Result<(Self, iroh::NodeId)> {
		let mls = Self::build_e2ee_identity(name);
		let mut delivery = Delivery::new().await?;
		let nid = delivery.id();

		let dirhash = hash_topic(groupid.clone());

		Ok((Self {
			mls,
			delivery,
			dirhash,
			nids
		}, nid))
	}

	/*
	Creates a group owner entrance
	*/
	pub async fn open(
		mut self,
		mailbox: mpsc::Sender<Message>,
		mut post: mpsc::Receiver<Message>
	) -> anyhow::Result<()> {
		let group = Arc::new(Mutex::new(self.mls.create_group(
			ExtensionList::new(),
			ExtensionList::new(),
			Some(MlsTime::now())
		)?));

		let (dir_runner, dir) = self.delivery.subscribe(
			self.dirhash.clone(),
			self.nids.unwrap_or(vec![])
		).await?;

		let chathash = gen_topic();
		let (chat_runner, chat) = self.delivery.subscribe(
			chathash.clone(),
			vec![]
		).await?;

		tokio::spawn(Self::greet(group.clone(), dir));
		dir_runner.spawn();

		tokio::spawn(Self::receive(
			group.clone(),
			chat.ready.clone(),
			chat.rx,
			mailbox
		));
		tokio::spawn(Self::deliver(
			group.clone(),
			chat.ready.clone(),
			chat.tx,
			post
		));
		chat_runner.spawn();

		Ok(())
	}

	/*
	pub async fn join(
		mut self,
		mailbox: mpsc::Sender<Message>,
		mut post: mpsc::Receiver<Message>,
		nids: Vec<iroh::NodeId>
	) -> anyhow::Result<()> {
		let kpkg = self.mls.generate_key_package_message(
			ExtensionList::new(),
			ExtensionList::new(),
			Some(MlsTime::now())
		)?;

		let chat = self.delivery.subscribe(self.chathash.clone(), vec![]).await?;
		let mut dir = self.delivery.subscribe(self.dirhash.clone(), vec![]).await?;

		// broadcast key package to directory
		dir.tx.send(kpkg.to_bytes()?).await.unwrap();

		// wait for posible welcome message
		while let Some(packet) = dir.rx.recv().await {
			match MlsMessage::from_bytes(&packet) {
				Ok(msg) => {
					if let MlsMessageDescription::Welcome {
						key_package_refs: refs,
						cipher_suite: suite
					} =  msg.description() {
						for kpg_ref in refs {
							println!("Referenced kpkg: {:#?}", *kpg_ref);
						}
					}
				},
				Err(why) => println!("Error parsing message in entrance.join(): {}", why)
			}
		}

		// let (group, info) = self.mls.join_group(None, welcome, None)?;

		Ok(())
	}*/

	async fn encrypt(
		group: &AsyncGroup,
		msg: Message
	) -> anyhow::Result<Packet> {
		let binding = msg.to_vec();
		let mlsmsg = group.lock().await.encrypt_application_message(
			binding.as_slice(),
			vec![]
		)?;

		Ok(mlsmsg.to_bytes()?)
	}

	async fn decrypt(
		group: &AsyncGroup,
		pack: Packet
	) -> anyhow::Result<ReceivedMessage> {
		let mlsmsg = MlsMessage::from_bytes(&pack)?;

		Ok(group.lock().await.process_incoming_message(mlsmsg)?)
	}

	/*
	Monitor public mls 'directory' topic for join requests,
	sending notifications to application to accept new
	members

	> Only group owners should run this
	*/
	// maybe make accept a future that the application can run?
	async fn greet(
		group: AsyncGroup,
		mut stream: TransportStream,
		// notifications: mpsc::Sender<()>
	) -> anyhow::Result<()> {
		stream.ready.notified().await;

		while let Some(packet) = stream.rx.recv().await {
		}
		Ok(())
	}

	/*
	Decrypt message packets from transport layer
	and route them to async application channel
	*/
	async fn receive(
		group: AsyncGroup,
		ready: Arc<Notify>,
		mut rx: mpsc::Receiver<Packet>,
		mailbox: mpsc::Sender<Message>
	) -> anyhow::Result<()> {
		ready.notified().await;
		// let mut rx = topic.await?;
		// while let Some(packet) = rx.recv().await {
		while let Some(packet) = rx.recv().await {
			// todo handle event error
			if let Ok(event) = Self::decrypt(&group, packet).await {
				match event {
					ReceivedMessage::ApplicationMessage(description) => {
						let data = description.data();
						let msg = Message::from_bytes(data)?;

						mailbox.send(msg).await?;
					},
					_ => ()
				}
			} else {
				// error
			}
		}
		Ok(())
	}

	/*
	Encrypt message packets from async application channel
	and send them over transport layer
	*/
	async fn deliver(
		group: AsyncGroup,
		ready: Arc<Notify>,
		tx: mpsc::Sender<Packet>,
		mut post: mpsc::Receiver<Message>,
	) -> anyhow::Result<()> {
		ready.notified().await;

		while let Some(msg) = post.recv().await {
			println!("Encrypting + delivering message: {:#?}", msg.body);
			let pack = Self::encrypt(&group, msg).await?;
			tx.send(pack).await?;
			// delivery.publish(&topickey, pack).await?;
			println!("Message delivered");
		}
		Ok(())
	}
}


