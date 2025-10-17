use dioxus::prelude::*;
use common::{
	model::Model,
	UserCommand,
	Message, MessageType
};

pub fn ChatApp() -> Element {
	let mut message = use_signal(|| String::new());
	let model = use_context::<Model>();
	rsx! {
		h1 { "Chat" }
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
