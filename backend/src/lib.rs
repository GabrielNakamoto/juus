use std::sync::Arc;
use tokio::sync::{mpsc, Notify, Mutex};
use log::info;
use common::UserCommand;

pub mod node;

use node::P2PNode;

#[derive(Clone)]
pub struct CommandRunner {
	notify: Arc<Notify>,
	ran_once: bool
}

impl std::ops::Drop for CommandRunner {
	fn drop(&mut self) {
		self.notify.notify_waiters();
	}
}

impl Default for CommandRunner {
	fn default() -> Self {
		Self::new()
	}
}

impl CommandRunner {
	pub fn new() -> Self {
		Self {
			notify: Arc::new(Notify::new()),
			ran_once: false
		}
	}

	pub fn run(&mut self, port: Option<u32>) {
		if self.ran_once {
			return;
		}
		self.ran_once = true;
		let notify = self.notify.clone();
		tokio::spawn(async move {
			Self::runtime(notify, port).await;
		});
	}
	
	async fn runtime(notify: Arc<Notify>, port: Option<u32>) -> anyhow::Result<()> {
		info!("Initializing p2p node...");
		let (mut ntx, nrx) = mpsc::channel(128);
		let mut node = P2PNode::new(nrx).await.unwrap();

		tokio::spawn(async move {
			node.listen(port).await
		});

		let mut rx = common::CMD_CHANNEL.rx.lock().await;
		while let Some(cmd) = rx.recv().await {
			info!("Backend received command: {:?}", cmd);
			match cmd {
				UserCommand::CreateGroup(groupname) => {
				},
				UserCommand::SendMessage(msg) => {
					if let Err(e) = ntx.send(msg).await {
						info!("Failed to send message to p2p thread: {}", e);
					}
				},
				_ => ()
			}
		}
		notify.notified().await;
		info!("Backend runtime closing down");
		Ok(())
	}
}

