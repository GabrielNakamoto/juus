use serde::{Serialize, Deserialize};
use iroh::NodeId;

pub type Packet = Vec<u8>;
pub type Hash = [u8; 32];

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

