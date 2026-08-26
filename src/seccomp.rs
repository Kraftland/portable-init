use thiserror::Error;

mod list;

pub use list::compile_syscall_list;

#[derive(Error, Debug)]
pub enum SeccompError {
	#[error("Could not create seccomp filter: {0:?}")]
	CreateFilterError(libseccomp::error::SeccompError),

	#[error("Could not add filter rule: {0:?}")]
	AddRuleError(libseccomp::error::SeccompError),

	#[error("Could not load filter into kernel: {0:?}")]
	LoadFilterError(libseccomp::error::SeccompError),

	#[error("Could not get notify fd: {0:?}")]
	GetFdError(libseccomp::error::SeccompError),
}

#[derive(Error,Debug)]
pub enum SyscallCompileError {

}

#[derive(Debug, Clone)]
pub struct SyscallList {
	pub deny_list: Vec<libseccomp::ScmpSyscall>,
	pub allow_list: Vec<libseccomp::ScmpSyscall>,
	pub debug_list: Vec<libseccomp::ScmpSyscall>,
	pub selective: Vec<libseccomp::ScmpSyscall>, // depends on lockdown
}

pub fn process_seccomp_unotify (
	fd: libseccomp::ScmpFd,
	cancel_token: tokio_util::sync::CancellationToken,
) {

	let errno_raw = - {
		use nix::errno::Errno;
		let err = Errno::ENOSYS;
		err as i32
	};

	let fake_allow: Vec<String> = vec![
		"chroot".into(),
		"capset".into(),
		"setfsuid".into(),
	];

	loop {
		let request = libseccomp::ScmpNotifReq::receive(fd);
		if cancel_token.is_cancelled() {
			return
		}
		let request = match request {
			Ok(val)	=> val,
			Err(e)	=> {
				eprintln!("Could not receive seccomp notification: {e:#?}");
				return
			}
		};

		let syscall_name = request.data.syscall.get_name();
		let syscall_name = match syscall_name {
			Ok(val)	=> val,
			Err(e)	=> {
				eprintln!("Could not resolve syscall: {:#?}", e);
				let response = libseccomp::ScmpNotifResp::new_continue(
					request.id,
					libseccomp::ScmpNotifRespFlags::empty(),
				);
				match response.respond(fd) {
					Ok(_)	=> {}
					Err(e)	=> {
						eprintln!("Could not respond to syscall: {e:#?}");
					}
				};
				continue;
			}
		};

		crate::logger::log_warn(
			format!(
				"PID {} performed illegal syscall: {} on architecture {:?}",
				&request.pid,
				syscall_name,
				&request.data.arch,
			)
		);

		let response = {
			if fake_allow.contains(&syscall_name) {
				libseccomp::ScmpNotifResp::new_val(
					request.id,
					0,
					libseccomp::ScmpNotifRespFlags::empty(),
				)
			} else {
				libseccomp::ScmpNotifResp::new_error(
					request.id,
					errno_raw,
					libseccomp::ScmpNotifRespFlags::empty(),
				)
			}
		};

		match response.respond(fd) {
			Ok(_)	=> {},
			Err(e)	=> {
				eprintln!("Could not filter system call: {e:#?}")
			},
		}
	}
}

pub async fn compile_filter (
	config_env:	std::sync::Arc<crate::envs::ConfigOpts>,
	syscall_list:	&SyscallList,
) -> Result<libseccomp::ScmpFilterContext, SeccompError> {
	let mut filter_result = match config_env.lockdown {
		true	=>	{
			let filter = libseccomp::ScmpFilterContext::new(
				libseccomp::ScmpAction::Notify,
				//libseccomp::ScmpAction::Log,
			);
			let mut filter = match filter {
				Ok(val) => val,
				Err(e) => {
					return Err(SeccompError::CreateFilterError(e));
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

// Loads a Secure Computing filter, does not spawn a unotify instance
pub fn load_seccomp_filter (
	filter_compiled: libseccomp::ScmpFilterContext,
) -> Result<libseccomp::ScmpFd, SeccompError> {
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
