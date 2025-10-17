use libp2p::{
	noise, tcp, yamux, gossipsub, mdns,
	swarm::{Swarm, NetworkBehaviour},
	futures::StreamExt
};

use std::{
	collections::hash_map::DefaultHasher,
	hash::{Hash, Hasher},
	time::Duration
};

use log::info;

#[derive(NetworkBehaviour)]
struct JuBehaviour {
	gossipsub: gossipsub::Behaviour,
	mdns: mdns::tokio::Behaviour
}

pub struct P2PNode {
	swarm: Swarm<JuBehaviour>
}

impl P2PNode {
	pub async fn new() -> anyhow::Result<Self> {
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

		Ok(Self {
			swarm
		})
	}

	pub async fn listen(&mut self) -> anyhow::Result<()> {
		let topic = gossipsub::IdentTopic::new("juus-messages");
		self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

		// let mut stdin = io::BufReader::new(io::stdin()).lines();
		self.swarm.listen_on("/ip4/0.0.0.0/udp/0/quic-v1".parse()?)?;
		self.swarm.listen_on("/ip4/0.0.0.0/tcp/0".parse()?)?;

		loop {
			tokio::select! {
				/*
				Ok(Some(line)) = stdin.next_line() => {
					if let Err(e) = swarm
						.behaviour_mut().gossipsub
						.publish(topic.clone(), line.as_bytes()) {
						info!("Gossipsub public error: {e:?}");
					}
				}*/
				event = self.swarm.select_next_some() => match event {
					_ => info!("P2P Swarm event: {:?}", event)
				}
			}
		}
	}
}
