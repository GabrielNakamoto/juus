use std::path::Path;
use iroh::NodeId;
use std::collections::HashMap;
use std::io::Write;
use mls_rs::{
	time::MlsTime,
	MlsMessage,
    Client,
	Group,
	// ReceivedMessage,
	client_builder::MlsConfig,
    identity::{SigningIdentity, basic::{BasicIdentityProvider, BasicCredential}},
    CipherSuite,
	ExtensionList
};
use serde::{Deserialize, Serialize};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
use mls_rs_crypto_openssl::OpensslCryptoProvider;

mod ds;
use ds::*;

use clap::Parser;
#[derive(Parser, Debug)]
enum Cmd {
	Open,
	Join { nid: String }
}

#[derive(Parser, Debug)]
struct Args {
	#[clap(short, long)]
	groupid: String,

	#[clap(short, long)]
	name: String,

	#[clap(subcommand)]
	command: Cmd
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
	pub body: MessageBody,
	pub nonce: [u8; 16]
}

impl Message {
	pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
		serde_json::from_slice(bytes).map_err(Into::into)
	}

	pub fn new(body: MessageBody) -> Self {
		Self {
			body,
			nonce: rand::random()
		}
	}

	pub fn to_vec(&self) -> Vec<u8> {
		serde_json::to_vec(self).expect("serde_json::to_vec")
	}
}

#[derive(Debug, Serialize, Deserialize)]
pub enum MessageBody {
	Text { from: NodeId, content: String },
	Introduce { from: NodeId, name: String },
}



fn build_e2ee_identity(name: String) -> Client<impl MlsConfig> {
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

/*
fn encrypt_message<C: MlsConfig>(group: &mut Group<C>, msg: Message) -> anyhow::Result<MlsMessage> {
 	let binding = msg.to_vec();

Ok(group.encrypt_application_message(binding.as_slice(), vec![])?)
}

fn decrypt_message<C: MlsConfig>(group: &mut Group<C>, bytes: Vec<u8>) -> anyhow::Result<ReceivedMessage> {
	let wrapper = MlsMessage::from_bytes(bytes.as_slice());

	Ok(group.process_incoming_message(wrapper)?)
}
*/

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let args = Args::parse();
	let mut delivery = Delivery::new().await?;

	let dir = Path::new("");
	let pre = String::from(args.name.clone());
	let mut file = tempfile::Builder::new()
		.prefix(&pre)
		.tempfile_in(dir)?;
	file.write_all(delivery.id().as_bytes())?;
	println!("Wrote nid to: {}", file.path().display());

	let e2ee_client = build_e2ee_identity(String::from("Gabriel"));

	let (mut group, nids) = match args.command {
		Cmd::Open => {
			let group = e2ee_client.create_group(
				ExtensionList::new(),
				ExtensionList::new(),
				Some(MlsTime::now())
			)?;

			(Some(group), vec![])
		},
		Cmd::Join { nid } => {
			use std::io::Read;

			let mut nf = std::fs::File::open(Path::new(&nid))?;
			let mut buf = [0; 32];
			nf.read_exact(&mut buf)?;
			(None, vec![NodeId::from_bytes(&buf)?])
		}
	};

	let handle = delivery.subscribe(&args.groupid, nids).await?;
	tokio::spawn(handle_loop(handle));

	let intro = Message::new(MessageBody::Introduce {
		from: delivery.id(),
		name: args.name.clone()
	});

	delivery.publish(&args.groupid, intro.to_vec()).await?;

	let (ltx, mut lrx) = tokio::sync::mpsc::channel(8);
	std::thread::spawn(move || input_loop(ltx));

	while let Some(txt) = lrx.recv().await {
		let msg = Message::new(MessageBody::Text {
			from: delivery.id(),
			content: txt.clone()
		});

		let mut bytes = msg.to_vec();
		if let Some(group) = &mut group {
			let encmsg = group.encrypt_application_message(bytes.as_slice(), vec![])?;
			bytes = encmsg.to_bytes()?;
		}
		
		println!("Sent: {}", txt);
		delivery.publish(&args.groupid, bytes).await?;
	}

	delivery.shutdown().await?;
	Ok(())
}

fn handle_msg(names: &mut HashMap<NodeId, String>, bytes: &Vec<u8>) -> anyhow::Result<()> {
	use hexhex::hex;

	let mlsmsg = MlsMessage::from_bytes(&bytes)?;
	println!("Msg description: {:?}", mlsmsg.description());
	match Message::from_bytes(&bytes)?.body {
		MessageBody::Introduce  { from, name } => {
			names.insert(from, name.clone());
			println!("Alias {} => {}", from, name);
		},
		MessageBody::Text { from, content } => {
			let rep = from.fmt_short().to_string();
			let name = names
				.get(&from)
				.unwrap_or_else(|| &rep);
			println!("Received msg from {}:\n{}", name, content);
		}
	}
	Ok(())
}

async fn handle_loop(mut stream: tokio::sync::mpsc::Receiver<Vec<u8>>) -> anyhow::Result<()> {
	println!("Initiated application message receive loop...");

	let mut names = HashMap::new();
	while let Some(bytes) = stream.recv().await {
		if let Err(e) = handle_msg(&mut names, &bytes) {
			println!("Failed to receive a message: {}", e);
		}
	}
	Ok(())
}

fn input_loop(line_tx: tokio::sync::mpsc::Sender<String>) -> anyhow::Result<()> {
	let mut buffer = String::new();

	let stdin = std::io::stdin();
	loop {
		stdin.read_line(&mut buffer)?;
		buffer.trim_end();

		line_tx.blocking_send(buffer.clone())?;
		buffer.clear();
	}
}
