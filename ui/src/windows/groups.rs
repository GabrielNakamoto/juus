use dioxus::prelude::*;

pub fn GroupsApp() -> Element {
	rsx! {
		button { "Join group" },
		button { "Create group" }
	}
}
