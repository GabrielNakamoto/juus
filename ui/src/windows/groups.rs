use dioxus::prelude::*;
use common::UserCommand;
use common::model::Model;
use log::info;

pub fn GroupsApp() -> Element {
	let mut groupname = use_signal(|| String::new());
	rsx! {
		/*
		button {
			"Join group"
		},
		*/
		button {
			onclick: move |event| {
				info!("Creating group: {}", groupname.read());
				let cmd = UserCommand::CreateGroup(groupname.read().clone());
				common::CMD_CHANNEL.tx.send(cmd).unwrap();
			},
			"Create group",
		},
		input {
			placeholder: "group name",
			oninput: move |event| *groupname.write()=event.value()
		}
	}
}
