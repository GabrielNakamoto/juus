// Delivery Service (DS)
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

pub struct Delivery {
	ep: Endpoint,
	router: Router,
	gossip: Gossip,
	publishers: HashMap<String, GossipSender>,
	receivers: HashMap<String, GossipReceiver>
}

fn hash_topic(key: String) -> [u8; 32] {
	let topic = key + "_juus";
	let mut hasher = Sha3_256::new();
	hasher.update(topic.as_bytes());
	hasher.finalize().into()
}

impl Delivery {
	pub async fn new() -> anyhow::Result<Self> {
		let skey = SecretKey::generate(&mut rand::rng());
		let ep = Endpoint::builder()
			.secret_key(skey)
			.discovery_n0()
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
			receivers: HashMap::new()
		})
	}

	pub fn id(&self) -> NodeId {
		self.ep.node_id()
	}

	pub async fn subscribe(
		&mut self,
		key: &String,
		nids: Vec<NodeId>
	) -> anyhow::Result<()>{
		let hash = hash_topic(key.clone());
		let tid = TopicId::from_bytes(hash);

		println!("Subscribing to topic: {}", key);
		println!("Bootstrap nodes: {:?}", nids);
		println!("Waiting for peers on topic: {}", tid);
		let topic = self.gossip.subscribe_and_join(tid, nids).await?;
		println!("Joined topic...");

		let (tx, rx) = topic.split();
		self.publishers.insert(key.clone(), tx);
		self.receivers.insert(key.clone(), rx);
		Ok(())
	}

	pub async fn publish(
		&mut self,
		key: &String,
		msg: Message
	) -> anyhow::Result<()> {
		let tx = &self.publishers.get(key).context("topic not found")?;

		tx.broadcast(msg.to_vec().into()).await?;
		Ok(())
	}

	pub async fn shutdown(&mut self) -> anyhow::Result<()> {
		self.router.shutdown().await?;
		Ok(())
	}

	async fn handle_topic_impl(mut rx: GossipReceiver) -> anyhow::Result<()> {
		let mut names = HashMap::new();
		while let Some(evt) = rx.try_next().await? {
			if let Event::Received(msg) = evt {
				match Message::from_bytes(&msg.content)?.body {
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
		Ok(())
	}

	pub async fn handle_topic(&mut self, key: &String) -> anyhow::Result<()> {
		let mut rx = self.receivers.remove(key).context("topic not found")?;
		println!("Spawned async handler task for topic: {}", key);
		tokio::spawn(Self::handle_topic_impl(rx));

		Ok(())
	}
}
