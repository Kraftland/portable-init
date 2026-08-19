pub enum SandboxStatus {
	/**
		Sandbox is running and tracking a certain amount of processes
	*/
	Ready { tracked_pid: usize },

	/**
		Sandbox is stopping
	*/
	Stopping,
}

impl std::fmt::Display for SandboxStatus {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match &self {
			SandboxStatus::Ready { tracked_pid }	=> {
				let content = {
					let mut string = String::new();

					string.push_str("Tracking ");

					if *tracked_pid > 1 {
						string.push_str("processes: ");
					} else {
						string.push_str("process: ");
					};

					string.push_str(&tracked_pid.to_string());
					string
				};

				f.write_str(&content)
			}
			SandboxStatus::Stopping			=> {
				f.write_str("Sandbox is stopping")
			}
		}
	}
}

pub mod systemd;

/**
	The public trait Init is used to initialise status subsystems.

	For example, systemd may need a "Ready status"
*/
pub trait Init {
	async fn initialise(&self) -> Result<(), Self::StatusError>;

	type StatusError;
}

/**
	The public trait UpdateStatus is used to update the SandboxStatus for a certain subsystem.
*/
pub trait UpdateStatus {
	async fn update(&self, status: &SandboxStatus) -> Result<(), Self::StatusError>;

	type StatusError;
}
