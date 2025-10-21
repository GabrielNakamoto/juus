use std::path::Path;
use iroh::NodeId;
use std::collections::HashMap;
use tokio::sync::mpsc;
use std::io::Write;
use serde::{Deserialize, Serialize};

mod ds;
mod message;
mod entrance;
use ds::*;
use message::*;
use entrance::Entrance;

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

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let args = Args::parse();
	/*
	let mut delivery = Delivery::new().await?;

	let dir = Path::new("");
	let pre = String::from(args.name.clone());
	let mut file = tempfile::Builder::new()
		.prefix(&pre)
		.tempfile_in(dir)?;
	file.write_all(delivery.id().as_bytes())?;
	println!("Wrote nid to: {}", file.path().display());
	*/

	let (tx, mailbox) = mpsc::channel(8);
	let (post, rx) = mpsc::channel(8);
	let entrance = match args.command {
		Cmd::Open => {
			let ent = Entrance::open(&args.name, &args.groupid, tx, rx).await?;
			ent
		},
		Cmd::Join { nid } => {
			let ent = Entrance::open(&args.name, &args.groupid, tx, rx).await?;
			ent
		}
	};

	/*
	let (mut group, nids) = match args.command {
		Cmd::Join { nid } => {
			use std::io::Read;

			let mut nf = std::fs::File::open(Path::new(&nid))?;
			let mut buf = [0; 32];
			nf.read_exact(&mut buf)?;
			(None, vec![NodeId::from_bytes(&buf)?])
		}
	};*/

	tokio::spawn(receive_loop(mailbox));

	let intro = Message::new(MessageBody::Introduce {
		from: entrance.id(),
		name: args.name.clone()
	});

	post.send(intro).await?;

	let (ltx, mut lrx) = tokio::sync::mpsc::channel(8);
	std::thread::spawn(move || input_loop(ltx));

	while let Some(txt) = lrx.recv().await {
		let msg = Message::new(MessageBody::Text {
			from: entrance.id(),
			content: txt.clone()
		});

		println!("Sent: {}", txt);
		post.send(msg).await?;
	}

	// delivery.shutdown().await?;
	Ok(())
}


async fn receive_loop(mut mailbox: mpsc::Receiver<Message>) {
	let mut names = HashMap::new();
	while let Some(msg) = mailbox.recv().await {
		match msg.body {
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
	}
}

fn input_loop(line_tx: mpsc::Sender<String>) -> anyhow::Result<()> {
	let mut buffer = String::new();

	let stdin = std::io::stdin();
	loop {
		stdin.read_line(&mut buffer)?;
		buffer.trim_end();

		line_tx.blocking_send(buffer.clone())?;
		buffer.clear();
	}
}
