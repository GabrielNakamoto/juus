// Delivery Service (DS)

/*
Should not deal with application specific abstractions
	* Given binary data to transmit
	* Sends received binary data over async channel for application handling

API:
	* Subscribe should spawn a task that handles topic I/O using tokio::select!
	* Returns a future that when run will wait until their are active nodes in
	  the topic and then return async channels to read and write bytes to the topic
*/

use hexhex::hex;
use futures_lite::stream::StreamExt;
use serde::{Deserialize, Serialize};
use iroh::{
	NodeId, Endpoint,
	protocol::Router,
	SecretKey
};
use iroh_gossip::{
	net::{Gossip},
	proto::TopicId,
	api::{Event, GossipTopic, GossipSender, GossipReceiver},
};
use tokio::sync::{mpsc, Notify};
use crate::types::*;
use std::sync::Arc;

pub struct Delivery {
	ep: Endpoint,
	router: Router,
	gossip: Gossip,
}

pub struct TransportStream {
	pub tx: mpsc::Sender<Packet>,
	pub rx: mpsc::Receiver<Packet>,
	pub ready: Arc<Notify>
}

pub struct TransportRunner {
	tx: GossipSender,
	rx: GossipReceiver,
	ready: Arc<Notify>,
	tr_in: mpsc::Receiver<Packet>,
	tr_out: mpsc::Sender<Packet>
}

impl TransportRunner {
	pub fn new(topic: GossipTopic) -> (Self, TransportStream) {
		let (tx, rx) = topic.split();

		let (apptx, tr_in) = mpsc::channel::<Packet>(8);
		let (tr_out, apprx) = mpsc::channel::<Packet>(8);

		let ready = Arc::new(Notify::new());
		let ready2 = ready.clone();

		(
			Self {tx, rx, ready, tr_in, tr_out},
			TransportStream {tx: apptx, rx: apprx, ready: ready2}
		)
	}

	pub fn spawn(mut self) {
		tokio::spawn(async move {
			self.rx.joined().await.unwrap();
			self.ready.notify_waiters();

			tokio::select! {
				Ok(Some(evt)) = self.rx.try_next() => {
					if let Event::Received(msg) = evt {
						self.tr_out.send(msg.content.to_vec()).await;
					}
				},
				Some(bytes) = self.tr_in.recv() => {
					if let Err(why) = self.tx.broadcast(bytes.into()).await {
						println!("Delivery broadcast error: {}", why);
					}
				}
			}
		});
	}
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
		})
	}

	pub fn id(&self) -> NodeId {
		self.ep.node_id()
	}

	pub async fn subscribe(
		&mut self,
		hash: Hash,
		nids: Vec<NodeId>,
	) -> anyhow::Result<(TransportRunner, TransportStream)> {
		let tid = TopicId::from_bytes(hash.clone());

		println!("Waiting for peers on topic: {}", hex(hash));
		println!("\tBootstrap nodes: {:?}", nids);

		let topic = self.gossip.subscribe(tid, nids).await?;

		println!("Joined topic...");

		Ok(TransportRunner::new(topic))
		/*
		let (tx, mut rx) = topic.split();

		let (apptx, mut bIn) = mpsc::channel::<Packet>(8);
		let (bOut, apprx) = mpsc::channel::<Packet>(8);

		let notify = Arc::new(Notify::new());
		let notify2 = notify.clone();

		tokio::spawn(async move {
			rx.joined().await.unwrap();
			notify.notify_waiters();

			tokio::select! {
				Ok(Some(evt)) = rx.try_next() => {
					if let Event::Received(msg) = evt {
						bOut.send(msg.content.to_vec()).await;
					}
				},
				Some(bytes) = bIn.recv() => {
					if let Err(why) = tx.broadcast(bytes.into()).await {
						println!("Delivery broadcast error: {}", why);
					}
				}
			}
		});
		*/

		// Ok(TransportHandle { tx: apptx, rx: apprx, ready: notify2 })
	}

	pub async fn shutdown(&mut self) -> anyhow::Result<()> {
		self.router.shutdown().await?;
		Ok(())
	}
}
