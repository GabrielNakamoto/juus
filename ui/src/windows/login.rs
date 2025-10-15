use dioxus::prelude::*;
use log::info;
use common::config::UserConfig;

#[derive(PartialEq, Props, Clone)]
pub struct LogInProps {
	pub logged_in: Signal<bool>,
	pub config: Option<UserConfig>
}

#[component]
pub fn LogInApp(mut props: LogInProps) -> Element {
	let mut username = use_signal(|| String::new());
	let mut password = use_signal(|| String::new());
	let signed_up = props.config.is_some();
	rsx! {
		if ! signed_up {
			h1 { "Make an account" }
		} else {
			h1 { "Log in to your account" }
		}
		form {
			onsubmit: move |event| {
				event.prevent_default();
				let result = if let Some(conf) = &props.config {
					conf.verify_password(password.read().clone()).unwrap()
				} else {
					let config = UserConfig::new(username.read().clone(), Some(password.read().clone())).unwrap();
					config.save().unwrap();

					true
				};
				*props.logged_in.write() = result;
			},
			if ! signed_up {
				input { name: "username", placeholder: "username",
					oninput: move |event| username.set(event.value()) }
			}
			input { name: "password", placeholder: "password",
				oninput: move |event| password.set(event.value())}
			button { r#type: "submit", if signed_up { "Log In" } else { "Sign up" } }
		}
	}
}

