/**
	Compile a Secure Computing filter

	Returns a filter to load, unotify is not used.
*/
pub async fn compile_filter (
	config_env:	std::sync::Arc<crate::envs::ConfigOpts>,
	syscall_list:	&super::SyscallList,
) -> Result<libseccomp::ScmpFilterContext, super::SeccompError> {
	use super::SeccompError;

	let mut filter_result = match config_env.seccomp_whitelist {
		true	=>	{
			let filter = libseccomp::ScmpFilterContext::new(
				libseccomp::ScmpAction::Errno(
					nix::errno::Errno::EPERM as i32
				),
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

	filter_result.add_arch(libseccomp::ScmpArch::Native)
		.map_err(SeccompError::AddRuleError)
		?;

	filter_result.set_ctl_tsync(true)
		.map_err(SeccompError::AddRuleError)
		?;

	match config_env.seccomp_whitelist {
		true => {
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
			// Handle deny list via the kernel, avoid spamming the journal and hurting performance

			for val in syscall_list.deny_list.iter() {
				filter_result.add_rule(
					libseccomp::ScmpAction::Errno(
						nix::errno::Errno::EPERM as i32
					),
					*val,
				)
					.map_err(SeccompError::AddRuleError)
					?;
			}
		}
	}

	match config_env.debugging {
		true	=> {
			if config_env.seccomp_whitelist {
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
		false	=> {
			if ! config_env.seccomp_whitelist {
				for val in syscall_list.debug_list.iter() {
					filter_result.add_rule(
					libseccomp::ScmpAction::Errno(
						nix::errno::Errno::EPERM as i32
					),
					*val,
				)
					.map_err(SeccompError::AddRuleError)
					?;
				}
			}
		}
	};

	Ok(filter_result)
}
