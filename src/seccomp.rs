use thiserror::Error;

#[derive(Error, Debug)]
pub enum SeccompError {
	#[error("Could not load seccomp filter: {0:?}")]
	EnvError(std::env::VarError),
	#[error("Unrecognised _portableLockdown environment {0:?}")]
	LockdownEnvError(String),
}

// Loads a Secure Computing filter
pub fn load_seccomp_filter () -> Result<(), SeccompError> {
	let mut ctx = seccomp
		::Context
		::default(seccomp::Action::Allow);

	let lockdown_env = std::env::var("_portableLockdown");

	let lockdown_env = match lockdown_env {
		Ok(val) => val,
		Err(e) => {
			if e == std::env::VarError::NotPresent {
				return Ok(())
			} else {
				return Err(
					SeccompError
						::EnvError
						(e))
			}
		}
	};

	let with_info: String = "with-info".to_string();
	let without_info: String = "without-info".to_string();

	let mut isLockdown: bool = false;
	let mut hasInfo: bool = false;

	if lockdown_env == with_info {
		isLockdown = true;
		hasInfo = true;
	} else if lockdown_env == without_info {
		isLockdown = true;
		hasInfo = false;
	} else {
		return Err(SeccompError::LockdownEnvError("Invalid _portableLockdown".to_string()));
	}



	Ok(())
}

