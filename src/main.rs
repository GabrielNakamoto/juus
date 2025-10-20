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

	let mut delivery = Delivery::new().await?;

	let dir = Path::new("");
	let pre = String::from(args.name.clone());
	let mut file = tempfile::Builder::new()
		.prefix(&pre)
		.tempfile_in(dir)?;
	file.write_all(delivery.id().as_bytes())?;
	println!("Wrote nid to: {}", file.path().display());

	let e2ee_client = build_e2ee_identity(String::from("Gabriel"));

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

	delivery.subscribe(&args.groupid, nids).await?;

	let intro = Message::new(MessageBody::Introduce {
		from: delivery.id(),
		name: args.name.clone()
	});

	delivery.publish(&args.groupid, intro).await?;
	delivery.handle_topic(&args.groupid);

	let (ltx, mut lrx) = tokio::sync::mpsc::channel(8);
	std::thread::spawn(move || input_loop(ltx));

	while let Some(txt) = lrx.recv().await {
		let msg = Message::new(MessageBody::Text {
			from: delivery.id(),
			content: txt.clone()
		});

		println!("Sent: {}", txt);
		delivery.publish(&args.groupid, msg).await?;
	}

	delivery.shutdown().await?;
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
