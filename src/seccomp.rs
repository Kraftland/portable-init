use thiserror::Error;
use crate::logger::LogMessage;

#[derive(Error, Debug)]
pub enum SeccompError {
	#[error("Could not create seccomp filter: {0:?}")]
	CreateFilterError(libseccomp::error::SeccompError),

	#[error("Could not add filter rule: {0:?}")]
	AddRuleError(libseccomp::error::SeccompError)
}

#[derive(Error,Debug)]
pub enum SyscallCompileError {

}

#[derive(Debug)]
pub struct SyscallList {
	pub deny_list: Vec<libseccomp::ScmpSyscall>,
	pub allow_list: Vec<libseccomp::ScmpSyscall>,
}

// Loads a Secure Computing filter
pub fn load_seccomp_filter (
	config_env: &crate::envs::ConfigOpts,
	syscall_list: &SyscallList) -> Result<(), SeccompError> {
	let filter = libseccomp::ScmpFilterContext::new(
		libseccomp::ScmpAction::Allow,
	);

	let mut filter = match filter {
		Ok(res)	=>	res,
		Err(e)	=>	{
			return Err(SeccompError::CreateFilterError(e));
		},
	};

	let filter_result = filter.add_arch(
		libseccomp::ScmpArch::Native
	);
	match filter_result {
		Ok(_)	=>	{},
		Err(e)	=>	{
			return Err(SeccompError::AddRuleError(e));
		},
	}

	let filter_result = match config_env.lockdown {
		true	=>	{
			filter.set_act_badarch(
				libseccomp::ScmpAction::Errno(
					std::io::Error::new(
						std::io::ErrorKind::Unsupported,
						"Architecture unsupported",
					)
						.raw_os_error()
						.unwrap()
			))
		}
		false	=>	{
			filter.set_act_badarch(libseccomp::ScmpAction::Allow)
		}
	};
	match filter_result {
		Ok(_)	=>	{},
		Err(e)	=>	{
			return Err(SeccompError::AddRuleError(e));
		},
	}


	Ok(())
}

pub fn compile_syscall_list(
	logtx: &tokio::sync::mpsc::Sender<LogMessage>,
) -> Result<SyscallList, SyscallCompileError> {
	struct SyscallByNames {
		async_io: Vec<String>, // @aio
		basic_io: Vec<String>, // @basic-io
		chown: Vec<String>,
		clock: Vec<String>,
	}

	let syscall_by_names = SyscallByNames {
		async_io: vec![
			"io_cancel".into(),
			"io_destroy".into(),
			"io_getevents".into(),
			"io_pgetevents".into(),
			"io_pgetevents_time64".into(),
		],
		basic_io: vec![
			"_llseek".into(),
			"close".into(),
			"close_range".into(),
			"dup".into(),
			"dup2".into(),
			"dup3".into(),
			"llseek".into(),
			"lseek".into(),
			"pread64".into(),
			"preadv".into(),
			"preadv2".into(),
			"pwrite64".into(),
			"pwritev".into(),
			"pwritev2".into(),
			"read".into(),
			"readv".into(),
			"write".into(),
			"writev".into(),
		],
		chown: vec![
			"chown".into(),
			"chown32".into(),
			"fchown".into(),
			"fchown32".into(),
			"fchownat".into(),
			"lchown".into(),
			"lchown32".into(),
		],
		clock: vec![
			"adjtimex".into(),
			"clock_adjtime".into(),
			"clock_adjtime64".into(),
			"clock_settime".into(),
			"clock_settime64".into(),
			"settimeofday".into(),
		]
	};

	let allowed_syscall_group = vec![
		syscall_by_names.async_io,
		syscall_by_names.basic_io,
		syscall_by_names.chown,
	];
	let denied_syscall_group: Vec<Vec<String>> = vec![
		syscall_by_names.clock,
	];

	let mut allowed_syscalls: Vec<libseccomp::ScmpSyscall> = vec![];
	let mut denied_syscalls: Vec<libseccomp::ScmpSyscall> = vec![];

	for val in allowed_syscall_group.iter() {
		for name in val.iter() {
			let syscall = get_syscall_by_name(&name, logtx);
			match syscall {
				Some(val)	=> {
					allowed_syscalls.push(val);
				}
				None		=> {}
			}
		}
	};

	for val in denied_syscall_group.iter() {
		for name in val.iter() {
			let syscall = get_syscall_by_name(&name, logtx);
			match syscall {
				Some(val)	=> {
					denied_syscalls.push(val);
				}
				None		=> {}
			}
		}
	}

	let ret = SyscallList{
		allow_list: allowed_syscalls,
		deny_list: denied_syscalls,
	};

	crate::logger::log(
		logtx,
		crate::logger::Loglevel::Debug,
		format!("Compiled seccomp allow list and deny list: {ret:#?}"));

	Ok(ret)

}

fn get_syscall_by_name(
	name: &String,
	logtx: &tokio::sync::mpsc::Sender<LogMessage>,
) -> Option<libseccomp::ScmpSyscall> {
	let result = libseccomp::ScmpSyscall::from_name(name);
	match result {
		Ok(val)	=>	Some(val),
		Err(e)	=>	{
			crate::logger::log(
				logtx,
				crate::logger::Loglevel::Warn,
				format!("Could not resolve syscall {name}: {e}"));
			None
		}
	}
}
