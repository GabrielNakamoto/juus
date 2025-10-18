use log::info;
use dioxus::prelude::*;
use std::path::Path;
use common::config::UserConfig;
use common::model::Model;
use common::EventNotification;
use windows::login::{LogInApp, LogInProps};
use dioxus_desktop::{Config, WindowBuilder};
use dioxus_desktop::tao::window::Fullscreen;

mod windows;

fn main() {
	env_logger::init();
	let config = Config::default()
		.with_window(
			WindowBuilder::new()
				.with_title("juus")
				.with_focused(true)
				.with_maximized(true)
				.with_resizable(false)
				// .with_fullscreen(Some(Fullscreen::Borderless(None)))
				//.with_decorations(false)
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

use clap::Parser;
#[derive(Parser)]
#[command(version = None, about = None, long_about = None)]
struct Args {
	#[arg(short, long)]
	port: u32
}

fn use_backend_model(config: UserConfig) -> ReadOnlySignal<Model> {
	let model = use_signal(move || Model {
		config,
		messages: Vec::new()
	});

	use_effect(move || {
		let mut model = model.clone();
		spawn(async move {
            let mut rx = common::EVT_CHANNEL.rx.lock().await;
            while let Some(evt) = rx.recv().await {
                info!("Received event notification from backend...");
                match evt {
                    EventNotification::SentMessage(msg) => {
                        model.write().messages.push(msg);
                    },
                    EventNotification::ReceivedMessage(msg) => {
                        model.write().messages.push(msg);
                    },
                    _ => ()
                }
            }
        });
	});

	ReadOnlySignal::new(model)
}

#[component]
fn App() -> Element {
// 0. Bootstrap backend threads
	use_hook(|| {
		let args = Args::parse();
		let mut runner = backend::CommandRunner::new();
		runner.run(Some(args.port));
		runner
	});
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

	let model = use_backend_model(config.unwrap());
	use_context_provider(|| model);

// 2. Ensure user is in a group with 1 or more people
	/*
	rsx! {
		windows::groups::GroupsApp { }
	}*/

// 3. Show chat page
	rsx! {
		windows::chat::ChatApp { }
	}
}
