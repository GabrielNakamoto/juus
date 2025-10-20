use iroh::{protocol::Router, Endpoint, SecretKey, NodeId};
use std::collections::HashMap;
use std::path::Path;
use iroh_gossip::{
	net::{Gossip},
	api::{Event, GossipReceiver},
	proto::{TopicId}
};
use n0_snafu::ResultExt;
use std::io::Write;
use sha3::{Digest, Sha3_256};
use hexhex::hex;
use serde::{Serialize, Deserialize};
use futures_lite::stream::StreamExt;
use mls_rs::{
	time::MlsTime,
    Client,
	client_builder::MlsConfig,
    identity::{SigningIdentity, basic::{BasicIdentityProvider, BasicCredential}},
    CipherSuite,
	ExtensionList
};
use ed25519_dalek::{SigningKey, VerifyingKey, Signature};
use mls_rs_crypto_openssl::OpensslCryptoProvider;

fn hash_topic(key: String) -> [u8; 32] {
	let topic = key + "_juus";
	let mut hasher = Sha3_256::new();
	hasher.update(topic.as_bytes());
	hasher.finalize().into()
}

mod ds;
use ds::*;

/*
#[derive(Debug, Serialize, Deserialize)]
struct Message {
	body: MessageBody,
	nonce: [u8; 16]
}

impl Message {
	fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
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
enum MessageBody {
	Text { from: NodeId, content: String },
	Introduce { from: NodeId, name: String },
	// EntryWay { owner: NodeId, question: String },
	// JoinRequest { from: NodeId, answer: String }, // answer to a question exchanged in EntryWay msg
	// Invitation { from: NodeId, ticket: Group }
}*/

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let args = Args::parse();

	let secret_key = SecretKey::generate(&mut rand::rng());
	let endpoint = Endpoint::builder()
		.secret_key(secret_key)
		.discovery_n0()
		.bind()
		.await?;
		// .discovery_dht()

	println!("Initialized endpoint with id: {}", endpoint.node_id());

	let dir = Path::new("");
	let pre = String::from(args.name.clone());
	let mut file = tempfile::Builder::new()
		.prefix(&pre)
		.tempfile_in(dir)?;
	file.write_all(endpoint.node_id().as_bytes())?;
	println!("Wrote nid to: {}", file.path().display());

	let gossip = Gossip::builder().spawn(endpoint.clone());

	// Routers job: Endpoint I/O -> dispatch to protocol handler
	let router = Router::builder(endpoint.clone())
		.accept(iroh_gossip::ALPN, gossip.clone())
		.spawn();

	let e2ee_client = build_e2ee_identity(String::from("Gabriel"));

	let hash = hash_topic(args.groupid);
	let tid = TopicId::from_bytes(hash);
	let (group, nids) = match args.command {
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

	println!("Waiting for peers on topic: {}", tid);
	let topic = gossip.subscribe_and_join(tid, nids).await?;
	let (tx, rx) = topic.split();

	{
		let intro = Message::new(MessageBody::Introduce {
			from: endpoint.node_id(),
			name: args.name.clone()
		});
		tx.broadcast(intro.to_vec().into()).await?;
	}
	
	tokio::spawn(subscribe_loop(rx));

	let (ltx, mut lrx) = tokio::sync::mpsc::channel(8);
	std::thread::spawn(move || input_loop(ltx));

	while let Some(txt) = lrx.recv().await {
		let msg = Message::new(MessageBody::Text {
			from: endpoint.node_id(),
			content: txt.clone()
		});

		println!("Sent: {}", txt);
		tx.broadcast(msg.to_vec().into()).await?;
	}

	router.shutdown().await?;
	Ok(())
}

fn input_loop(line_tx: tokio::sync::mpsc::Sender<String>) -> anyhow::Result<()> {
	let mut buffer = String::new();

	let stdin = std::io::stdin();
	loop {
		stdin.read_line(&mut buffer)?;

		line_tx.blocking_send(buffer.clone())?;
		buffer.clear();
	}
}

async fn subscribe_loop(mut rx: GossipReceiver) -> anyhow::Result<()> {
	let mut names = HashMap::new();
	while let Some(evt) = rx.try_next().await? {
		if let Event::Received(msg) = evt {
			match Message::from_bytes(&msg.content)?.body {
				MessageBody::Introduce { from, name } => {
					names.insert(from, name.clone());
					println!("> Recording {} as {}", from, name);
				},
				MessageBody::Text { from, content } => {
					let rep = from.fmt_short().to_string();
					let name = names
						.get(&from)
						.unwrap_or_else(|| &rep);
					println!("Received msg from {}:\n{}", name, content);
				}
			}
		}
	}
	Ok(())
}
