use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::Write;
use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
/*
Json user config,
created after authenticated sign up
*/
#[derive(PartialEq, Clone, Deserialize, Serialize)]
pub struct UserConfig {
	pub username: String,
	pub group_members: Option<Vec<String>>,
	pub password_hash: Option<String> // if no password hash then user doesnt care about authenticated access
	// metadata?
	// auth data?
}

static CONFIG_PATH: &str = "user.json";

impl UserConfig {
	pub fn new(username: String, password: Option<String>) -> anyhow::Result<Self> {
		let password_hash = match password {
			Some(pwd) => Some(Self::hash_password(&pwd)?),
			None => None
		};

		Ok(Self {
			username,
			group_members: None,
			password_hash
		})
	}

	pub fn save(&self) -> anyhow::Result<()> {
		let data = serde_json::to_string(&self)?;
		let mut file = File::create(CONFIG_PATH)?;

		Ok(file.write_all(data.as_bytes())?)
	}

	pub fn requires_password(&self) -> bool {
		self.password_hash.is_some()
	}

	fn hash_password(password: &str) -> anyhow::Result<String> {
		let salt = SaltString::generate(&mut OsRng);
		let argon2 = Argon2::default();
		
		let hash = argon2.hash_password(password.as_bytes(), &salt)
			.map_err(|e| anyhow::anyhow!("Failed to hash password: {}", e))?;

		Ok(hash.to_string())
	}

	pub fn verify_password(&self, password: String) -> anyhow::Result<bool> {
		let hashed_password = &self.password_hash;
		match hashed_password {
			Some(hash) => {
				let parsed_hash = PasswordHash::new(&hash)
					.map_err(|e| anyhow::anyhow!("Failed to deserialize hashed password: {}", e))?;

				let result = Argon2::default().verify_password(password.as_bytes(), &parsed_hash);
				Ok(result.is_ok())
			},
			None => Ok(false)
		}
	}
}
