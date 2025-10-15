use log::info;
use dioxus::prelude::*;
use std::path::Path;
use common::config::UserConfig;
use windows::login::{LogInApp, LogInProps};
use dioxus_desktop::{Config, WindowBuilder};

mod windows;

fn main() {
	env_logger::init();
	let config = Config::default()
		.with_window(
			WindowBuilder::new()
				.with_title("juus")
				.with_focused(true)
				.with_resizable(false)
				//.with_decorations(false)
				// .with_fullscreen()
		);
	dioxus::LaunchBuilder::desktop()
		.with_cfg(config)
		.launch(App);
}

fn get_user_config(path: &Path) -> anyhow::Result<UserConfig> {
	let data = std::fs::read_to_string(path)?;
	serde_json::from_str::<UserConfig>(&data)
		.map_err(|e| anyhow::anyhow!("Failed to decode user config: {}", e))
}

#[component]
fn App() -> Element {
// 1. Ensure user is logged in / signed up
	let config_path = Path::new("user.json");

	let (logged_in, config) = if ! config_path.exists() {
		info!("No config found");
		(false, None)
	} else {
		let conf = get_user_config(&config_path).unwrap();
		info!("Found user config");
		(! conf.requires_password(), Some(conf))
	};

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

// 2. Ensure user is in a group with 1 or more people
	rsx! {
		windows::groups::GroupsApp { }
	}

// 3. Show chat page
}
