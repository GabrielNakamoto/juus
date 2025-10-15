use log::info;
use dioxus::prelude::*;
use std::path::Path;
use common::config::UserConfig;

fn main() {
	env_logger::init();
	dioxus::launch(App);
}

fn get_user_config(path: &Path) -> anyhow::Result<UserConfig> {
	let data = std::fs::read_to_string(path)?;
	serde_json::from_str::<UserConfig>(&data)
		.map_err(|e| anyhow::anyhow!("Failed to decode user config: {}", e))
}

#[component]
fn App() -> Element {
	let config_path = Path::new("user.json");

	let (logged_in, config) = if ! config_path.exists() {
		info!("No config found");
		(false, None)
	} else {
		let conf = get_user_config(&config_path).unwrap();
		info!("Found user config");
		(! conf.requires_password(), Some(conf))
	};
	info!("Log in required: {}", logged_in);

	let signed_up = config.is_some();
	let authenticated = use_signal(|| logged_in);
	if ! *authenticated.read() {
		return rsx! {
			LogInApp { 
				logged_in: authenticated.clone(),
				config,
			}
		}
	}

	rsx! {
		LoggedInApp {}
	}
}

#[derive(PartialEq, Props, Clone)]
struct LogInProps {
	logged_in: Signal<bool>,
	config: Option<UserConfig>
}

#[component]
fn LogInApp(mut props: LogInProps) -> Element {
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
				println!("Log in / sign up result: {}", result);
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

#[component]
fn LoggedInApp() -> Element {
	rsx! {
		h1 { "Your logged in!" }
	}
}
