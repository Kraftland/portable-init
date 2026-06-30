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
	pub debug_list: Vec<libseccomp::ScmpSyscall>,
	pub selective: Vec<libseccomp::ScmpSyscall>, // depends on lockdown
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

	// TODO: handle debug, denied and allowed syscall list

	Ok(())
}

pub fn compile_syscall_list(
	logtx: &tokio::sync::mpsc::Sender<LogMessage>,
) -> Result<SyscallList, SyscallCompileError> {
	struct SyscallByNames {
		async_io:	Vec<String>, // @aio
		basic_io:	Vec<String>, // @basic-io
		chown:		Vec<String>,
		clock:		Vec<String>,
		debug:		Vec<String>,
		fs_op:		Vec<String>, // @file-system
		io_ev:		Vec<String>, // @io-event
		ipc:		Vec<String>,
		keyring:	Vec<String>,
		memlock:	Vec<String>,
		module:		Vec<String>,
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
		],
		debug: vec![
			"lookup_dcookie".into(),
			"perf_event_open".into(),
			"pidfd_getfd".into(),
			"ptrace".into(),
			"rtas".into(),
			"s390_runtime_instr".into(),
			"sys_debug_setcontext".into(),
		],
		fs_op: vec![
			"access".into(),
			"chdir".into(),
			"chmod".into(),
			"close".into(),
			"creat".into(),
			"faccessat".into(),
			"faccessat2".into(),
			"fallocate".into(),
			"fchdir".into(),
			"fchmod".into(),
			"fchmodat".into(),
			"fchmodat2".into(),
			"fcntl".into(),
			"fcntl64".into(),
			"fgetxattr".into(),
			"file_getattr".into(),
			"file_setattr".into(),
			"flistxattr".into(),
			"fremovexattr".into(),
			"fsetxattr".into(),
			"fstat".into(),
			"fstat64".into(),
			"fstatat".into(),
			"fstatat64".into(),
			"fstatfs".into(),
			"fstatfs64".into(),
			"ftruncate".into(),
			"ftruncate64".into(),
			"futimesat".into(),
			"getcwd".into(),
			"getdents".into(),
			"getdents64".into(),
			"getxattr".into(),
			"getxattrat".into(),
			"inotify_add_watch".into(),
			"inotify_init".into(),
			"inotify_init1".into(),
			"inotify_rm_watch".into(),
			"lgetxattr".into(),
			"link".into(),
			"linkat".into(),
			"listmount".into(),
			"listxattr".into(),
			"listxattrat".into(),
			"llistxattr".into(),
			"lremovexattr".into(),
			"lsetxattr".into(),
			"lstat".into(),
			"lstat64".into(),
			"mkdir".into(),
			"mkdirat".into(),
			"mknod".into(),
			"mknodat".into(),
			"newfstat".into(),
			"newfstatat".into(),
			"oldfstat".into(),
			"oldlstat".into(),
			"oldstat".into(),
			"open".into(),
			"open_tree".into(),
			"openat".into(),
			"openat2".into(),
			"readlink".into(),
			"readlinkat".into(),
			"removexattr".into(),
			"removexattrat".into(),
			"rename".into(),
			"renameat".into(),
			"renameat2".into(),
			"rmdir".into(),
			"setxattr".into(),
			"setxattrat".into(),
			"stat".into(),
			"stat64".into(),
			"statfs".into(),
			"statfs64".into(),
			"statmount".into(),
			"statx".into(),
			"symlink".into(),
			"symlinkat".into(),
			"truncate".into(),
			"truncate64".into(),
			"unlink".into(),
			"unlinkat".into(),
			"utime".into(),
			"utimensat".into(),
			"utimensat_time64".into(),
			"utimes".into(),
		],
		io_ev: vec![
			"_newselect".into(),
			"epoll_create".into(),
			"epoll_create1".into(),
			"epoll_ctl".into(),
			"epoll_ctl_old".into(),
			"epoll_pwait".into(),
			"epoll_pwait2".into(),
			"epoll_wait".into(),
			"epoll_wait_old".into(),
			"eventfd".into(),
			"eventfd2".into(),
			"poll".into(),
			"ppoll".into(),
			"ppoll_time64".into(),
			"pselect6".into(),
			"pselect6_time64".into(),
			"select".into(),
		],
		ipc: vec![
			"ipc".into(),
			"memfd_create".into(),
			"mq_getsetattr".into(),
			"mq_notify".into(),
			"mq_open".into(),
			"mq_timedreceive".into(),
			"mq_timedreceive_time64".into(),
			"mq_timedsend".into(),
			"mq_timedsend_time64".into(),
			"mq_unlink".into(),
			"msgctl".into(),
			"msgget".into(),
			"msgrcv".into(),
			"msgsnd".into(),
			"pipe".into(),
			"pipe2".into(),
			"process_madvise".into(),
			"process_vm_readv".into(),
			"process_vm_writev".into(),
			"semctl".into(),
			"semget".into(),
			"semop".into(),
			"semtimedop".into(),
			"semtimedop_time64".into(),
			"shmat".into(),
			"shmctl".into(),
			"shmdt".into(),
			"shmget".into(),
		],

		// Not sure if this is suitable for userspace
		keyring: vec![
			"add_key".into(),
			"keyctl".into(),
			"request_key".into(),
		],
		memlock: vec![
			"mlock".into(),
			"mlock2".into(),
			"mlockall".into(),
			"munlock".into(),
			"munlockall".into(),
		],
		module: vec![
			"delete_module".into(),
			"finit_module".into(),
			"init_module".into(),
		]
	};

	let allowed_syscall_group = vec![
		syscall_by_names.async_io,
		syscall_by_names.basic_io,
		syscall_by_names.chown,
		syscall_by_names.fs_op,
		syscall_by_names.io_ev,
		syscall_by_names.ipc,
		syscall_by_names.memlock,
	];
	let denied_syscall_group: Vec<Vec<String>> = vec![
		syscall_by_names.clock,
		syscall_by_names.module,
	];
	let debug_syscall_group: Vec<Vec<String>> = vec![
		syscall_by_names.debug,
	];
	let lockdown_syscall_group: Vec<Vec<String>> = vec![
		syscall_by_names.keyring,
	];

	let mut allowed_syscalls: Vec<libseccomp::ScmpSyscall> = vec![];
	let mut denied_syscalls: Vec<libseccomp::ScmpSyscall> = vec![];
	let mut debug_syscalls: Vec<libseccomp::ScmpSyscall> = vec![];
	let mut lockdown_syscalls: Vec<libseccomp::ScmpSyscall> = vec![];

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

	for val in debug_syscall_group.iter() {
		for name in val.iter() {
			let syscall = get_syscall_by_name(&name, logtx);
			match syscall {
				Some(val)	=> {
					debug_syscalls.push(val);
				}
				None		=> {}
			}
		}
	}

	for val in lockdown_syscall_group.iter() {
		for name in val.iter() {
			let syscall = get_syscall_by_name(&name, logtx);
			match syscall {
				Some(val)	=> {
					lockdown_syscalls.push(val);
				}
				None		=> {}
			}
		}
	}

	let ret = SyscallList{
		allow_list: allowed_syscalls,
		deny_list: denied_syscalls,
		debug_list: debug_syscalls,
		selective: lockdown_syscalls,
	};

	crate::logger::log(
		logtx,
		crate::logger::Loglevel::Debug,
		format!("Compiled seccomp allow list and deny list: {ret:#?}"));
	crate::logger::log(
		logtx,
		crate::logger::Loglevel::Debug,
		format!(
			"{} allowed syscalls, {} denied syscalls, {} debug syscalls and {} lockdown syscalls",
			ret.allow_list.len(),
			ret.deny_list.len(),
			ret.debug_list.len(),
			ret.selective.len(),
		));

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
				format!("{e:#?}"));
			None
		}
	}
}
