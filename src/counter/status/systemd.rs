
/**
	The SystemdStatus represents an implementation of the systemd notify protocol
*/
pub struct SystemdStatus {}

impl super::Init for SystemdStatus {
	async fn initialise(&self) -> Result<(), Self::StatusError> {
		systemd::daemon::notify(false, vec![("READY", "1")].iter())
			.map_err(SystemdError::NotifyError)
			?;
		Ok(())
	}

	type StatusError = SystemdError;
}

impl super::UpdateStatus for SystemdStatus {
	async fn update(&self, status: &super::SandboxStatus) -> Result<(), Self::StatusError> {
		systemd::daemon::notify(
			false,
			vec![
				(
					systemd::daemon::STATE_STATUS,
					&status
						.to_string(),
				)
			].iter(),
		)
			.map_err(SystemdError::NotifyError)
			?;

		Ok(())
	}

	type StatusError = SystemdError;
}

#[derive(thiserror::Error, Debug)]
pub enum SystemdError {
	#[error("Error sending systemd notification: {0:#?}")]
	NotifyError(systemd::Error),
}
