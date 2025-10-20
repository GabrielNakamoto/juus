// Delivery Service (DS)
use futures_lite::stream::StreamExt;
use serde::{Deserialize, Serialize};
use sha3::Sha3_256;
use iroh::{
	NodeId, Endpoint,
	protocol::Router,
	SecretKey
};
use sha3::Digest;
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

// TODO: mutex sync
struct TopicStream {
	tx: GossipSender,
	rx: GossipReceiver
}

impl TopicStream {
	fn from_topic(topic: GossipTopic) -> Self {
		let (tx, rx) = topic.split();
		Self {
			tx,
			rx
		}
	}
}

pub struct DS {
	ep: Endpoint,
	router: Router,
	gossip: Gossip,
	topics: HashMap<String, TopicStream>
}

fn hash_topic(key: String) -> [u8; 32] {
	let topic = key + "_juus";
	let mut hasher = Sha3_256::new();
	hasher.update(topic.as_bytes());
	hasher.finalize().into()
}

impl DS {
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

		Ok(Self {
			ep,
			router,
			gossip,
			topics: HashMap::new()
		})
	}

	pub async fn subscribe(
		&mut self,
		key: String,
		nids: Vec<NodeId>
	) -> anyhow::Result<()>{
		let hash = hash_topic(key.clone());
		let tid = TopicId::from_bytes(hash);

		let topic = self.gossip.subscribe_and_join(tid, nids).await?;
		let stream = TopicStream::from_topic(topic);
		self.topics.insert(key, stream);
		Ok(())
	}

	pub async fn publish(
		&mut self,
		key: &String,
		msg: Message
	) -> anyhow::Result<()> {
		let topic = &self.topics.get(key).context("topic not found")?;

		topic.tx.broadcast(msg.to_vec().into()).await?;
		Ok(())
	}

	pub async fn handle_topic(&mut self, key: &String) -> anyhow::Result<()> {
		let mut topic = &mut self.topics.get_mut(key).context("topic not found")?;
		let mut names = HashMap::new();

		while let Some(evt) = topic.rx.try_next().await? {
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
}
