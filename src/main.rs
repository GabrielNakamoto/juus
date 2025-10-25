use std::path::Path;
use std::collections::HashMap;
use tokio::sync::mpsc;
use std::io::Write;
use iroh::EndpointId;

mod ds;
mod types;
mod entrance;
use ds::*;
use types::*;
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
	env_logger::init();
	let args = Args::parse();

	let (tx, mailbox) = mpsc::channel(8);
	let (post, rx) = mpsc::channel(8);

	let nid = match args.command {
		Cmd::Open => {
			let (mut entrance, nid) = Entrance::new(&args.name, &args.groupid, None).await?;
			tokio::spawn(entrance.open(tx, rx));

			println!("Spawned entrance open");
			nid
		},
		Cmd::Join { nid } => {
			use std::io::Read;

			let mut nf = std::fs::File::open(Path::new(&nid))?;
			let mut buf = [0; 32];
			nf.read_exact(&mut buf)?;

			let nids = vec![EndpointId::from_bytes(&buf)?];

			let (mut entrance, nid) = Entrance::new(&args.name, &args.groupid, Some(nids)).await?;
			tokio::spawn(entrance.join(tx, rx));

			nid
		}
	};

	let dir = Path::new("");
	let pre = String::from(args.name.clone());
	let mut file = tempfile::Builder::new()
		.prefix(&pre)
		.tempfile_in(dir)?;
	file.write_all(nid.as_bytes())?;
	println!("Wrote nid to: {}", file.path().display());

	tokio::spawn(receive_loop(mailbox));

	let intro = Message::new(MessageBody::Introduce {
		from: nid,
		name: args.name.clone()
	});

	post.send(intro).await?;

	let (ltx, mut lrx) = tokio::sync::mpsc::channel(8);
	std::thread::spawn(move || input_loop(ltx));

	while let Some(txt) = lrx.recv().await {
		let msg = Message::new(MessageBody::Text {
			from: nid,
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
