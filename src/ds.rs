// Delivery Service (DS)

/*
Should not deal with application specific abstractions
	* Given binary data to transmit
	* Sends received binary data over async channel for application handling
*/

use futures_lite::stream::StreamExt;
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use iroh::{
	NodeId, Endpoint,
	protocol::Router,
	SecretKey
};
use iroh_gossip::{
	net::{Gossip},
	proto::TopicId,
	api::{
		Event,
		GossipTopic,
		GossipSender,
		GossipReceiver
	}
};
use anyhow::Context;
use std::collections::HashMap;
use tokio::sync::mpsc;

pub struct Delivery {
	ep: Endpoint,
	router: Router,
	gossip: Gossip,
	publishers: HashMap<String, GossipSender>,
}

fn hash_topic(key: String) -> [u8; 32] {
	let topic = key + "_juus";
	let mut hasher = Sha3_256::new();
	hasher.update(topic.as_bytes());
	hasher.finalize().into()
}

// static DIRECT_ALPN: &str = "juus/iroh/0.1";

impl Delivery {
	pub async fn new() -> anyhow::Result<Self> {
		let skey = SecretKey::generate(&mut rand::rng());
		let ep = Endpoint::builder()
			.secret_key(skey)
			.discovery_dht()
			.bind()
			.await?;

		let gossip = Gossip::builder()
			.spawn(ep.clone());

		let router = Router::builder(ep.clone())
			.accept(iroh_gossip::ALPN, gossip.clone())
			.spawn();

		println!("Initialized iroh endpoint with id: {}", ep.node_id());
		Ok(Self {
			ep,
			router,
			gossip,
			publishers: HashMap::new(),
		})
	}

	pub fn id(&self) -> NodeId {
		self.ep.node_id()
	}

	pub async fn subscribe(
		&mut self,
		key: &String,
		nids: Vec<NodeId>,
		wait: bool
	) -> anyhow::Result<mpsc::Receiver<Vec<u8>>>{
		let hash = hash_topic(key.clone());
		let tid = TopicId::from_bytes(hash);

		println!("Subscribing to topic: {}", key);
		println!("Bootstrap nodes: {:?}", nids);
		println!("Waiting for peers on topic: {}", tid);

		let topic = if wait {
			self.gossip.subscribe_and_join(tid, nids).await?
		} else {
			self.gossip.subscribe(tid, nids).await?
		};

		println!("Joined topic...");

		let (tx, mut rx) = topic.split();
		let (out, handle) = mpsc::channel(8);

		self.publishers.insert(key.clone(), tx);
		tokio::spawn(Self::handle_topic_impl(out, rx));

		Ok(handle)
	}

	pub async fn publish(
		&mut self,
		key: &String,
		bytes: Vec<u8>
	) -> anyhow::Result<()> {
		let tx = &self.publishers.get(key).context("topic not found")?;

		tx.broadcast(bytes.into()).await?;
		Ok(())
	}

	pub async fn shutdown(&mut self) -> anyhow::Result<()> {
		self.router.shutdown().await?;
		Ok(())
	}

	/*
	pub async fn send_direct(&mut self, nid: NodeId, msg: Message) -> anyhow::Result<()> {
		let conn = self.ep.connect(nid, DIRECT_ALPN.as_bytes());
		Ok(())
	}
	*/

	async fn handle_topic_impl(stream: mpsc::Sender<Vec<u8>>, mut rx: GossipReceiver) -> anyhow::Result<()> {
		println!("Initiated async topic handler...");
		while let Some(evt) = rx.try_next().await? {
			if let Event::Received(msg) = evt {
				stream.send(msg.content.to_vec()).await;
			}
		}
		Ok(())
	}
}
