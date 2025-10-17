use libp2p::{
	noise, tcp, yamux, gossipsub, mdns,
	swarm::{Swarm, SwarmEvent, NetworkBehaviour},
	futures::StreamExt
};

use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	time::Duration
};

use anyhow::Context;
use common::{Message, MessageType};
use tokio::sync::mpsc;

use log::{info, warn};

#[derive(NetworkBehaviour)]
struct JuBehaviour {
	gossipsub: gossipsub::Behaviour,
	mdns: mdns::tokio::Behaviour
}

type MessageStream = mpsc::Receiver<Message>;

pub struct P2PNode {
	swarm: Swarm<JuBehaviour>,
	messages: MessageStream,
	topic: gossipsub::IdentTopic
}

impl P2PNode {
	pub async fn new(stream: MessageStream) -> anyhow::Result<Self> {
		let mut swarm = libp2p::SwarmBuilder::with_new_identity()
			.with_tokio()
			.with_tcp(
				tcp::Config::default(),
				noise::Config::new,
				yamux::Config::default,
			)?
			.with_quic()
			.with_behaviour(|key| {
				let message_id_fn = |message: &gossipsub::Message| {
					let mut s = DefaultHasher::new();
					message.data.hash(&mut s);
					gossipsub::MessageId::from(s.finish().to_string())
				};

				let config = gossipsub::ConfigBuilder::default()
					.heartbeat_interval(Duration::from_secs(10))
					.validation_mode(gossipsub::ValidationMode::Strict)
					.message_id_fn(message_id_fn)
					.build()?;

				let gossip = gossipsub::Behaviour::new(
					gossipsub::MessageAuthenticity::Signed(key.clone()),
					config
				)?;

				let mdns =
					mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())?;
				Ok(JuBehaviour { gossipsub: gossip, mdns, })
			})?
			.build();

			let topic = gossipsub::IdentTopic::new("juus-messages");
			swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

		Ok(Self {
			swarm,
			messages: stream,
			topic
		})
	}

	pub fn publish_message(&mut self, data: Vec<u8>) -> anyhow::Result<()> {
		self.swarm
			.behaviour_mut().gossipsub
			.publish(self.topic.clone(), &data[..])
			.context("Failed to publish message to gossipsub topic")?;
		Ok(())
	}

	pub async fn listen(&mut self, port: Option<u32>) -> anyhow::Result<()> {
		// let mut stdin = io::BufReader::new(io::stdin()).lines();
		self.swarm.listen_on(format!("/ip4/0.0.0.0/udp/{}/quic-v1", port.unwrap_or(0)).parse()?)?;
		self.swarm.listen_on(format!("/ip4/0.0.0.0/tcp/{}", port.unwrap_or(0)).parse()?)?;

		loop {
			tokio::select! {
				Some(msg) = self.messages.recv() => {
					let data = match msg.kind {
						MessageType::Text(txt) => txt.as_bytes().to_vec()
					};
					if let Err(e) = self.publish_message(data) {
						info!("Gossipsub public error: {e:?}");
					}
				},
				event = self.swarm.select_next_some() => match event {
					SwarmEvent::Behaviour(JuBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
						for (p_id, maddr) in list {
							info!("mDNS discovered peer with id: {}", p_id);
							self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&p_id);
						}
					},
					SwarmEvent::Behaviour(JuBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
						for (p_id, maddr) in list {
							info!("mDNS peer expired with id: {}", p_id);
							self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&p_id);
						}
					},
					SwarmEvent::NewListenAddr { address, .. } => {
						info!("Local node is listening on address: {}", address);
					},
					SwarmEvent::Behaviour(JuBehaviourEvent::Gossipsub(gossipsub::Event::Message{
						propagation_source: p_id,
						message_id: m_id,
						message
					})) => {
						info!("Received message from peer [{}]:\n{}", p_id,
							String::from_utf8_lossy(&message.data));
					},
					_ => info!("P2P Swarm event: {:?}", event)
				}
			}
		}
	}
}
