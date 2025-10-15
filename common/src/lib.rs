pub mod config;
use std::sync::Arc;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use tokio::sync::mpsc;


// Commands sent from UI
pub enum UserCommand {
	// StartTyping -> ui only event?
	// SendMessage(Message),
	InviteMember(String),
	CreateGroup(String),
}

pub struct ChannelWrapper {
	pub tx: mpsc::UnboundedSender<UserCommand>,
	// Arc + Mutex for multi thread
	// mutability
	pub rx: Arc<Mutex<mpsc::UnboundedReceiver<UserCommand>>>
}

pub static CMD_CHANNEL: Lazy<ChannelWrapper> = Lazy::new(|| {
	let (tx, mut rx) = mpsc::unbounded_channel();

	ChannelWrapper {
		tx,
		rx: Arc::new(Mutex::new(rx))
	}
});
/*
use serde::{Deserialize, Serialize};


enum MessageType {
	Text(String),
	/*
	Image,
	File
	*/
}

// message type includes metadata such as reply m(essage)_id, attached image etc...
// message should include metadata + message type enum?
struct Message {
	kind: MessageType,
	reply_id: Option<MessageId>
}

// Notifications to be send to UI
enum EventNotification {
	MessageSent,
	ReceivedMessage,
}
*/
