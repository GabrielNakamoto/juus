use std::sync::Arc;
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, mpsc};
use bincode::{Encode, Decode};
use crate::model::Model;

pub mod config;
pub mod model;

// Commands sent from UI
#[derive(Debug)]
pub enum UserCommand {
	// StartTyping -> ui only event?
	SendMessage(Message),
	InviteMember(String),
	CreateGroup(String),
}

// Notifications to be send to UI
pub enum EventNotification {
	SentMessage(Message),
	ReceivedMessage(Message),
}

#[derive(Clone)]
pub struct ChannelWrapper<T> {
	pub tx: mpsc::UnboundedSender<T>,
	// Arc + Mutex for multi thread
	// mutability
	pub rx: Arc<Mutex<mpsc::UnboundedReceiver<T>>>
}

pub static CMD_CHANNEL: Lazy<ChannelWrapper<UserCommand>> = Lazy::new(|| {
	let (tx, mut rx) = mpsc::unbounded_channel();

	ChannelWrapper {
		tx, rx: Arc::new(Mutex::new(rx))
	}
});

pub static EVT_CHANNEL: Lazy<ChannelWrapper<EventNotification>> = Lazy::new(|| {
	let (tx, mut rx) = mpsc::unbounded_channel();

	ChannelWrapper {
		tx, rx: Arc::new(Mutex::new(rx))
	}
});

#[derive(Debug, Clone, Encode, Decode)]
pub enum MessageType {
	Text(String),
	/*
	Image,
	File
	*/
}

#[derive(Debug, Clone, Encode, Decode)]
pub struct Message {
	pub kind: MessageType,
	pub sender: String
}

impl Message {
	pub fn new(kind: MessageType, sender: &String) -> Self {
		Self {
			kind,
			sender: sender.clone()
		}
	}
}

/*
// message type includes metadata such as reply m(essage)_id, attached image etc...
// message should include metadata + message type enum?
struct Message {
	kind: MessageType,
	reply_id: Option<MessageId>
}

*/
