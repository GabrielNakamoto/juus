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
use log::{info, debug, error};
use futures_lite::stream::StreamExt;
use serde::{Deserialize, Serialize};
use iroh::{
	EndpointId, Endpoint,
	protocol::Router,
	SecretKey
};
use iroh_gossip::{
	net::{Gossip, GOSSIP_ALPN},
	proto::TopicId,
	api::{Event, GossipTopic, GossipSender, GossipReceiver},
};
use tokio::sync::{mpsc, Notify};
use crate::types::*;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DsTicket {
	pub topic_hash: Hash,
	pub bootstrap: Vec<EndpointId>
}

pub struct TransportStream {
	pub tx: mpsc::Sender<Packet>,
	pub rx: mpsc::Receiver<Packet>,
	pub ready: Arc<Notify>
}

pub struct TransportRunner {
	meta: Hash,
	gossip: Arc<Gossip>,
	tid: TopicId,
	bootstrap: Vec<EndpointId>,
	ready: Arc<Notify>,
	tr_in: mpsc::Receiver<Packet>,
	tr_out: mpsc::Sender<Packet>
}

impl TransportRunner {
	pub fn new(
		gossip: Arc<Gossip>,
		tid: TopicId,
		bootstrap: Vec<EndpointId>,
		hash: Hash
	) -> (Self, TransportStream) {
		let (apptx, tr_in) = mpsc::channel::<Packet>(8);
		let (tr_out, apprx) = mpsc::channel::<Packet>(8);

		let ready = Arc::new(Notify::new());
		let ready2 = ready.clone();

		(
			Self { meta: hash, gossip, tid, bootstrap, ready, tr_in, tr_out },
			TransportStream {tx: apptx, rx: apprx, ready: ready2}
		)
	}

	pub fn spawn(mut self) {
		tokio::spawn(async move {
			info!("Transport runner task spawned for topic hash: {}", hex(&self.meta[..8]));
			info!("Bootstraping with: {:#?}", self.bootstrap);
			info!("Waiting for peers...");

			let topic = self.gossip.subscribe(
				self.tid,
				self.bootstrap
			).await.unwrap();

			info!("Subscribed to topic...");
			let (tx, mut rx) = topic.split();
			
			rx.joined().await.unwrap();
			info!("Found peers...");

			self.ready.notify_waiters();

			loop {
				tokio::select! {
					Ok(Some(evt)) = rx.try_next() => {
						if let Event::Received(msg) = evt {
							self.tr_out.send(msg.content.to_vec()).await;
						}
					},
					Some(bytes) = self.tr_in.recv() => {
						if let Err(why) = tx.broadcast(bytes.into()).await {
							error!("Delivery broadcast error: {}", why);
						}
					}
				}
			}
		});
	}
}

pub struct Delivery {
	router: Router,
	gossip: Arc<Gossip>,
}

impl Delivery {
	pub async fn new() -> anyhow::Result<Self> {
		let skey = SecretKey::generate(&mut rand::rng());
		let mdns = iroh::discovery::mdns::MdnsDiscovery::builder();
		let ep = Endpoint::builder()
			.secret_key(skey)
			.discovery(mdns)
			.alpns(vec![GOSSIP_ALPN.to_vec()])
			.bind()
			.await?;

		let gossip = Gossip::builder()
			.spawn(ep.clone());

		let id = ep.id();
		let router = Router::builder(ep)
			.accept(GOSSIP_ALPN, gossip.clone())
			.spawn();

		info!("Initialized iroh endpoint with id: {}", id.fmt_short());
		Ok(Self {
			router,
			gossip: Arc::new(gossip),
		})
	}

	pub fn id(&self) -> EndpointId {
		self.router.endpoint().id()
	}

	pub async fn subscribe(
		&mut self,
		hash: Hash,
		nids: Vec<EndpointId>,
	) -> anyhow::Result<(TransportRunner, TransportStream)> {
		let tid = TopicId::from_bytes(hash.clone());
		// let topic = self.gossip.subscribe(tid, nids).await?;

		Ok(TransportRunner::new(self.gossip.clone(), tid, nids, hash))
	}

	pub async fn shutdown(&mut self) -> anyhow::Result<()> {
		self.router.shutdown().await?;
		Ok(())
	}
}
