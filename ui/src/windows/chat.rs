use dioxus::prelude::*;
use common::{
	model::Model,
	UserCommand,
	Message, MessageType
};

pub fn ChatApp() -> Element {
	let mut message = use_signal(|| String::new());
	let state = use_context::<ReadOnlySignal<Model>>();
	let model = state.read().clone();
	rsx! {
		h1 { "Chat" }
		div {
			for msg in model.messages {
				if let MessageType::Text(text) = msg.kind {
					{ format!("{}: {}", msg.sender, text) }
				}
			}
		}
		form {
			onsubmit: move |event| {
				event.prevent_default();
				let msg = Message::new(
					MessageType::Text(message.read().to_string()),
					&model.config.username
				);
				message.set(String::new());
				let cmd = UserCommand::SendMessage(msg);
				common::CMD_CHANNEL.tx.send(cmd).unwrap();
			},
			input { value: "{message}", placeholder: "message", oninput: move |evt| message.set(evt.value()), }
			button { r#type: "submit", "Send" }
		}
	}
}
