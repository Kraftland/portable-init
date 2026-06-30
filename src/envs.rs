use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnvsError {
	#[error("Unrecognised {0:?} environment {1:?}")]
	InvalidEnvError(String, String),
	#[error("Malformed environment variable: {0:?}")]
	NonUnicodeError(std::env::VarError)
}

#[derive(Debug)]
pub struct ConfigOpts {
	pub lockdown:		bool,
	pub has_flatpak_info:	bool,
	pub debugging:		bool,
}

pub fn get_configurations() -> Result<ConfigOpts, EnvsError> {
	let is_lockdown: bool;
	let has_flatpak_info: bool;

	let lockdown_env = std::env::var("_portableLockdown");

	let lockdown_env = match lockdown_env {
		Ok(val) => val,
		Err(e) => {
			if e == std::env::VarError::NotPresent {
				"".to_string()
			} else {
				return Err(
					EnvsError::NonUnicodeError(e)
				)
			}
		}
	};

	let with_info: String = "with-info".to_string();
	let without_info: String = "without-info".to_string();

	if lockdown_env == with_info {
		is_lockdown = true;
		has_flatpak_info = true;
	} else if lockdown_env == without_info {
		is_lockdown = true;
		has_flatpak_info = false;
	} else if lockdown_env == String::from("") {
		is_lockdown = false;
		has_flatpak_info = false;
	} else {
		return Err(EnvsError::InvalidEnvError(
			String::from("_portableLockdown"),
			lockdown_env,
		));
	}

	let mut is_debugging: bool = false;
	let debug_env = std::env::var("_portableAllowDebugging");
	match debug_env {
		Ok(val) => {
			if val == "1" {
				is_debugging = true;
			}
		}
		Err(_) => {}
	};

	Ok(ConfigOpts {
		lockdown: is_lockdown,
		has_flatpak_info: has_flatpak_info,
		debugging: is_debugging,
	})
}
