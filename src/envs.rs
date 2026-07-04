use thiserror::Error;
use std::ffi::OsString;
use serde::{Deserialize,Serialize};

#[derive(Error, Debug)]
pub enum EnvsError {
	#[error("Unrecognised {0:?} environment {1:#?}")]
	InvalidEnvError(String, String),

	#[error("Unrecognised {0:?} environment {1:#?}")]
	MalformedEnvError(String, std::env::VarError),

	#[error("Failed to get sandbox ID: {0:#?}")]
	AppIDError(std::env::VarError),

	#[error("Malformed environment variable: {0:#?}")]
	NonUnicodeError(std::env::VarError),

	#[error("Failed to decode _portableHelperExtraFiles: {0:#?}: {1:#?}")]
	PassFilesError(String, serde_json::Error),

	#[error("Invalid arguments: {0:#?}")]
	InvalidArgsError(String),
}

#[derive(Debug, Clone)]
pub struct ConfigOpts {
	pub lockdown:		bool,
	pub has_flatpak_info:	bool,
	pub debugging:		bool,
	pub sandbox_id:		String,

	// Origin -> dest
	pub file_map:		std::collections::HashMap<OsString, OsString>,

	pub inhibit:		bool,

	pub target:		OsString,
	pub args:		Vec<OsString>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct PassFiles {
	file_map:		std::collections::HashMap<OsString, OsString>
}

fn get_pass_files_env() -> Result<PassFiles, EnvsError> {
	let files_json = std::env::var("_portableHelperExtraFiles");
	let files_map: std::collections::HashMap<OsString,OsString> = std::collections::HashMap::new();
	let files_json = match files_json {
		Ok(val)	=> val,
		Err(e)	=> {
			if e == std::env::VarError::NotPresent {
				return Ok(
					PassFiles {
						file_map: files_map,
					},
				)
			} else {
				return Err(EnvsError::NonUnicodeError(e));
			}
		}
	};
	let deserialised: Result<PassFiles, serde_json::Error> = serde_json::from_str(&files_json);
	match deserialised {
		Ok(val)	=> Ok(val),
		Err(e)	=> {
			Err(EnvsError::PassFilesError(files_json, e))
		}
	}
}

pub fn get_configurations() -> Result<ConfigOpts, EnvsError> {
	let passed_files = match get_pass_files_env() {
		Ok(val)	=>	val,
		Err(e)	=>	return Err(e),
	};

	let has_inhibit = match std::env::var("_portableInhibit") {
		Ok(val)	=> {
			if val == "1" {
				true
			} else {
				false
			}
		},
		Err(e)	=> {
			if e == std::env::VarError::NotPresent {
				false
			} else {
				return Err(
					EnvsError::NonUnicodeError(e),
				);
			}
		}
	};

	let is_lockdown: bool;
	let has_info: bool;

	let app_id: String;

	match std::env::var("appID") {
		Ok(val)	=> {app_id = val}
		Err(e)	=> {
			return Err(EnvsError::AppIDError(e));
		}
	}

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





	let target = {
		match std::env::var_os("_portableLaunchTarget") {
			Some(v)	=> {v}
			None	=> {
				return Err(
					EnvsError::InvalidEnvError(
						"_portableLaunchTarget".into(),
						"None".into(),
					)
				);
			}
		}
	};

	let args = {
		let mut os_args = std::env::args_os();
		let _exec_name = os_args.next(); // looks like the first next call returns index 0?
		let mut args: Vec<OsString> = vec![];
		if os_args.len() > 1 {
			loop {
				match os_args.next() {
					Some(v)	=> {args.push(v);}
					None	=> {break}
				}
			}
		};
		args
	};

	Ok(ConfigOpts {
		lockdown: is_lockdown,
		has_flatpak_info: has_info,
		debugging: is_debugging,
		sandbox_id: app_id,
		file_map: passed_files.file_map,
		inhibit: has_inhibit,
		target: target,
		args: args,
	})
}
