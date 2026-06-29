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





	Ok(())
}

