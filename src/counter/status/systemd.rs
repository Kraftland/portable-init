/**
	The SystemdStatus represents an implementation of the systemd notify protocol
*/
pub struct SystemdStatus {
}

impl super::Init for SystemdStatus {
	async fn initialise(&self) -> Result<(), Self::StatusError> {
		let raw_fd = unsafe {
			libc::syscall(
				libc::SYS_pidfd_open,
				std::process::id(),
				libc::PIDFD_NONBLOCK,
			)
		};

		let fd = if raw_fd < 0 {
			return Err(
				SystemdError::PidfdError(std::io::Error::last_os_error())
			);
		} else {
			raw_fd as std::os::fd::RawFd
		};

		let state = {
			let mut vec = vec![];
			vec.push(
				libsystemd::daemon::NotifyState::Ready,
			);
			vec.push(
				libsystemd::daemon::NotifyState::Other(String::from("MAINPIDFD=1"))
			);
			vec
		};

		libsystemd::daemon::notify_with_fds(
			false,
			&state,
			&vec![fd],
		)
			.map_err(SystemdError::NotifyError)
			?;

		// systemd::daemon::pid_notify_with_fds(pid, unset_environment, state, fds)

		// systemd::daemon::notify(
		// 	false,
		// 	vec![
		// 		("READY", "1"),
				//("NOTIFYACCESS", "main"), // Reset NotifyAccess
		// 		("MAINPIDFDID", "1"),
		// 	].iter(),
		// )
		// 	.map_err(SystemdError::NotifyError)
		// 	?;

		Ok(())
	}

	type StatusError = SystemdError;
}

impl super::UpdateStatus for SystemdStatus {
	async fn update(&self, status: &super::SandboxStatus) -> Result<(), Self::StatusError> {
		libsystemd::daemon::notify(
			false,
			&vec![
				libsystemd::daemon::NotifyState::Status(
					status.to_string()
				)
			],
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
	NotifyError(libsystemd::errors::SdError),

	#[error("Error obtaining PIDFD: {0:#?}")]
	PidfdError(std::io::Error),
}
