/**
	Compile a Secure Computing filter
*/
pub async fn compile_filter (
	config_env:	std::sync::Arc<crate::envs::ConfigOpts>,
	syscall_list:	&super::SyscallList,
) -> Result<libseccomp::ScmpFilterContext, super::SeccompError> {
	use super::SeccompError;

	let mut filter_result = match config_env.seccomp_whitelist {
		true	=>	{
			let filter = libseccomp::ScmpFilterContext::new(
				libseccomp::ScmpAction::Notify,
				//libseccomp::ScmpAction::Log,
			);
			let mut filter = match filter {
				Ok(val) => val,
				Err(e) => {
					return Err(super::SeccompError::CreateFilterError(e));
				}
			};
			let result = filter.set_act_badarch(
				libseccomp::ScmpAction::KillThread,
			);

			match result {
				Ok(_) => {},
				Err(e) => {
					return Err(SeccompError::AddRuleError(e));
				}
			};

			filter
		}
		false	=>	{
			let filter = libseccomp::ScmpFilterContext::new(
				libseccomp::ScmpAction::Allow,
			);
			let mut filter = match filter {
				Ok(val) => val,
				Err(e) => {
					return Err(SeccompError::CreateFilterError(e));
				}
			};
			let result = filter.set_act_badarch(libseccomp::ScmpAction::Allow);
			match result {
				Ok(_) => {},
				Err(e) => {
					return Err(SeccompError::AddRuleError(e));
				}
			};

			filter
		}
	};

	match filter_result.add_arch(libseccomp::ScmpArch::Native) {
		Ok(_)	=>	{},
		Err(e)	=>	{
			return Err(SeccompError::AddRuleError(e));
		},
	};

	filter_result.set_ctl_tsync(true)
		.map_err(SeccompError::AddRuleError)
		?;



	match config_env.lockdown {
		true => {
			//println!("Appending allow list: {:?}", &syscall_list.allow_list);
			for val in syscall_list.allow_list.iter() {
				let result = filter_result.add_rule(
					libseccomp::ScmpAction::Allow,
					*val,
				);
				match result {
					Ok(_)	=> {},
					Err(e)	=> {
						return Err(SeccompError::AddRuleError(e))
					},
				}
			};
		}
		false => {
			for val in syscall_list.deny_list.iter() {
				let result = filter_result.add_rule(
					libseccomp::ScmpAction::Notify,
					*val,
				);
				match result {
					Ok(_)	=> {},
					Err(e)	=> {
						return Err(SeccompError::AddRuleError(e))
					},
				}
			};
		}
	}

	match config_env.debugging {
		true => {
			if config_env.lockdown {
				for val in syscall_list.debug_list.iter() {
					let result = filter_result.add_rule(
						libseccomp::ScmpAction::Allow,
						*val,
					);
					match result {
						Ok(_)	=> {},
						Err(e)	=> {
							return Err(SeccompError::AddRuleError(e))
						},
					}
				}
			}

		}
		false => {
			if ! config_env.lockdown {
				for val in syscall_list.debug_list.iter() {
					let result = filter_result.add_rule(
						libseccomp::ScmpAction::Notify,
						*val,
					);
					match result {
						Ok(_)	=> {},
						Err(e)	=> {
							return Err(SeccompError::AddRuleError(e))
						},
					}
				}
			}
		}
	};
	Ok(filter_result)
}

/// Loads a Secure Computing filter, does not spawn a unotify instance
pub fn load_seccomp_filter (
	filter_compiled: libseccomp::ScmpFilterContext,
) -> Result<libseccomp::ScmpFd, super::SeccompError> {
	use super::SeccompError;
	match filter_compiled.load() {
		Ok(_)	=> {},
		Err(e)	=> return Err(SeccompError::LoadFilterError(e))
	};

	let result = filter_compiled.get_notify_fd();
	match result {
		Ok(fd)	=> Ok(fd),
		Err(e)	=> return Err(SeccompError::GetFdError(e))
	}
}
