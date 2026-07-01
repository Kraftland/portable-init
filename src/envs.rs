use thiserror::Error;

#[derive(Error, Debug)]
pub enum EnvsError {
	#[error("Unrecognised {0:?} environment {1:?}")]
	InvalidEnvError(String, String),
	#[error("Malformed environment variable: {0:?}")]
	NonUnicodeError(std::env::VarError)
}

#[derive(Debug, Clone)]
pub struct ConfigOpts {
	pub lockdown:		bool,
	pub has_flatpak_info:	bool,
	pub debugging:		bool,
}

pub fn get_configurations() -> Result<ConfigOpts, EnvsError> {
	let is_lockdown: bool;
	let has_info: bool;

	let info_env = std::env::var("_portableHasFlatpakInfo");
	match info_env {
		Ok(val)	=> {
			if val == "1" {
				has_info = true
			} else {
				return Err(
					EnvsError::InvalidEnvError(
						"_portableHasFlatpakInfo".into(),
						val,
					)
				);
			}
		}
		Err(e)	=> {
			if e == std::env::VarError::NotPresent {
				has_info = false
			} else {
				return Err(
					EnvsError::NonUnicodeError(e)
				)
			}
		}
	}

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
	} else if lockdown_env == without_info {
		is_lockdown = true;
	} else if lockdown_env == String::from("") {
		is_lockdown = false;
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
		has_flatpak_info: has_info,
		debugging: is_debugging,
	})
}
