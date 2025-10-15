use crate::config::UserConfig;
/*
Contains state data that the frontend
requires to render ui
*/

#[derive(Clone)]
pub struct Model {
	pub config: UserConfig
}
