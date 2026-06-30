use thiserror::Error;
use crate::logger::LogMessage;

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

#[derive(Debug)]
pub struct SyscallList {
	pub deny_list: Vec<libseccomp::ScmpSyscall>,
	pub allow_list: Vec<libseccomp::ScmpSyscall>,
	pub debug_list: Vec<libseccomp::ScmpSyscall>,
	pub selective: Vec<libseccomp::ScmpSyscall>, // depends on lockdown
}

pub fn process_seccomp_unotify (
	fd: libseccomp::ScmpFd,
	logtx: &tokio::sync::mpsc::Sender<LogMessage>,
) {
	let execve_id = libseccomp::ScmpSyscall::from_name("execve");
	let execve_id = match execve_id {
		Ok(val) => val,
		Err(e)	=> {
			crate::logger::log_sync(
				&logtx,
				crate::logger::Loglevel::Fatal,
				format!("Could not resolve execve syscall ID: {e:#?}"),
			);
			return;
		}
	};

	let raw_eperm_err = std::io::Error::from(
		std::io::ErrorKind::PermissionDenied,
	).raw_os_error();

	let raw_eperm_err = match raw_eperm_err {
		Some(val)	=> val,
		None	=> {
			crate::logger::log_sync(
				&logtx,
				crate::logger::Loglevel::Fatal,
				format!("Could not resolve EPERM integer: None"));
				return;
		}
	};

	loop {
		let request = libseccomp::ScmpNotifReq::receive(fd);
		let request = match request {
			Ok(val)	=> val,
			Err(e)	=> {
				crate::logger::log_sync(
					&logtx,
					crate::logger::Loglevel::Fatal,
					format!("Could not receive seccomp notification: {e:#?}"));
				return
			}
		};
		if request.data.syscall == execve_id {
			// TODO: filter execve
			let response = libseccomp::ScmpNotifResp::new_continue(
				request.id,
				libseccomp::ScmpNotifRespFlags::empty(),
			);
			match response.respond(fd) {
				Ok(_)	=> {},
				Err(e)	=> {
					crate::logger::log_sync(
						&logtx,
						crate::logger::Loglevel::Warn,
						format!(
							"Error filtering syscall: {e:#?}",
						),
					);
				},
			}
			continue;
		}

		let syscall_name = request.data.syscall.get_name();
		let syscall_name = match syscall_name {
			Ok(val)	=> val,
			Err(e)	=> {
				format!("unresolved syscall ({:#?})", e)
			}
		};
		crate::logger::log_sync(
			&logtx,
			crate::logger::Loglevel::Warn,
			format!(
				"PID {} performed illegal system call {}",
				request.id,
				syscall_name,
			),
		);

		let response = libseccomp::ScmpNotifResp::new_error(
			request.id,
			raw_eperm_err,
			libseccomp::ScmpNotifRespFlags::empty(),
		);
		match response.respond(fd) {
			Ok(_)	=> {},
			Err(e)	=> {
				crate::logger::log_sync(
					&logtx,
					crate::logger::Loglevel::Warn,
					format!(
						"Error filtering syscall: {e:#?}",
					),
				);
			},
		}
	}
}

// Loads a Secure Computing filter
pub fn load_seccomp_filter (
	config_env: &crate::envs::ConfigOpts,
	syscall_list: &SyscallList) -> Result<libseccomp::ScmpFd, SeccompError> {

	let mut filter_result = match config_env.lockdown {
		true	=>	{
			let filter = libseccomp::ScmpFilterContext::new(
				libseccomp::ScmpAction::Notify,
			);
			let mut filter = match filter {
				Ok(val) => val,
				Err(e) => {
					return Err(SeccompError::CreateFilterError(e));
				}
			};
			let result = filter.set_act_badarch(
				libseccomp::ScmpAction::Errno(1));

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

	let filter_result = filter_result.add_arch(
		libseccomp::ScmpArch::Native
	);
	let filter_result = match filter_result {
		Ok(v)	=>	{v},
		Err(e)	=>	{
			return Err(SeccompError::AddRuleError(e));
		},
	};



	match config_env.lockdown {
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
	}

	let result = filter_result.set_ctl_nnp(true);
	match result {
		Ok(_)	=> {},
		Err(e)	=> return Err(SeccompError::AddRuleError(e))
	};

	let result = filter_result.precompute();
	match result {
		Ok(_)	=> {},
		Err(e)	=> return Err(SeccompError::LoadFilterError(e))
	};

	let result = filter_result.load();
	match result {
		Ok(_)	=> {},
		Err(e)	=> return Err(SeccompError::LoadFilterError(e))
	};

	let result = filter_result.get_notify_fd();
	match result {
		Ok(fd)	=> Ok(fd),
		Err(e)	=> Err(SeccompError::GetFdError(e))
	}
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
		mount:		Vec<String>,
		network:	Vec<String>, // @network-io
		obsolete:	Vec<String>,
		pkey:		Vec<String>, // memory protection keys
		raw_io:		Vec<String>,
		reboot:		Vec<String>,
		resources:	Vec<String>,
		swap:		Vec<String>,
		sync:		Vec<String>,
		process:	Vec<String>,
		process_notify:	Vec<String>,
		setuid:		Vec<String>,
		signal:		Vec<String>,
		timer:		Vec<String>,
		other:		Vec<String>, // uncategorised, always allowed syscalls
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
		],
		mount: vec![
			"fsconfig".into(),
			"fsmount".into(),
			"fsopen".into(),
			"fspick".into(),
			"mount".into(),
			"mount_setattr".into(),
			"move_mount".into(),
			"open_tree_attr".into(),
			"pivot_root".into(),
			"umount".into(),
			"umount2".into(),
		],
		network: vec![
			"accept".into(),
			"accept4".into(),
			"bind".into(),
			"connect".into(),
			"getpeername".into(),
			"getsockname".into(),
			"getsockopt".into(),
			"listen".into(),
			"recv".into(),
			"recvfrom".into(),
			"recvmmsg".into(),
			"recvmmsg_time64".into(),
			"recvmsg".into(),
			"send".into(),
			"sendmmsg".into(),
			"sendmsg".into(),
			"sendto".into(),
			"setsockopt".into(),
			"shutdown".into(),
			"socket".into(),
			"socketcall".into(),
			"socketpair".into(),
		],
		obsolete: vec![
			"_sysctl".into(),
			"afs_syscall".into(),
			"bdflush".into(),
			"break".into(),
			"create_module".into(),
			"ftime".into(),
			"get_kernel_syms".into(),
			"getpmsg".into(),
			"gtty".into(),
			"idle".into(),
			"lock".into(),
			"mpx".into(),
			"prof".into(),
			"profil".into(),
			"putpmsg".into(),
			"query_module".into(),
			"security".into(),
			"sgetmask".into(),
			"ssetmask".into(),
			"stime".into(),
			"stty".into(),
			"sysfs".into(),
			"tuxcall".into(),
			"ulimit".into(),
			"uselib".into(),
			"ustat".into(),
			"vserver".into(),
		],
		pkey: vec![
			"pkey_alloc".into(),
			"pkey_free".into(),
			"pkey_mprotect".into(),
		],
		raw_io: vec![
			"ioperm".into(),
			"iopl".into(),
			"pciconfig_iobase".into(),
			"pciconfig_read".into(),
			"pciconfig_write".into(),
			"s390_pci_mmio_read".into(),
			"s390_pci_mmio_write".into(),
		],
		reboot: vec![
			"kexec_file_load".into(),
			"kexec_load".into(),
			"reboot".into(),
		],
		resources: vec![
			"ioprio_set".into(),
			"mbind".into(),
			"migrate_pages".into(),
			"move_pages".into(),
			"nice".into(),
			"sched_setaffinity".into(),
			"sched_setattr".into(),
			"sched_setparam".into(),
			"sched_setscheduler".into(),
			"set_mempolicy".into(),
			"set_mempolicy_home_node".into(),
			"setpriority".into(),
			"setrlimit".into(),
		],
		swap: vec![
			"swapon".into(),
			"swapoff".into(),
		],
		sync: vec![
			"fdatasync".into(),
			"fsync".into(),
			"msync".into(),
			"sync".into(),
			"sync_file_range".into(),
			"sync_file_range2".into(),
			"syncfs".into(),
		],
		process: vec![
			"capget".into(),
			"clone".into(),
			"clone3".into(),
			"fork".into(),
			"getrusage".into(),
			"kill".into(),
			"pidfd_open".into(),
			"pidfd_send_signal".into(),
			"prctl".into(),
			"rt_sigqueueinfo".into(),
			"rt_tgsigqueueinfo".into(),
			"swapcontext".into(),
			"tgkill".into(),
			"times".into(),
			"tkill".into(),
			"unshare".into(),
			"vfork".into(),
			"wait4".into(),
			"waitid".into(),
			"waitpid".into(),
		],
		process_notify: vec![
			"setns".into(),
			"execveat".into(),
			"execve".into(),
		],
		setuid: vec![
			"setgid".into(),
			"setgid32".into(),
			"setgroups".into(),
			"setgroups32".into(),
			"setregid".into(),
			"setregid32".into(),
			"setresgid".into(),
			"setresgid32".into(),
			"setresuid".into(),
			"setresuid32".into(),
			"setreuid".into(),
			"setreuid32".into(),
			"setuid".into(),
			"setuid32".into(),
		],
		signal: vec![
			"rt_sigaction".into(),
			"rt_sigpending".into(),
			"rt_sigprocmask".into(),
			"rt_sigsuspend".into(),
			"rt_sigtimedwait".into(),
			"rt_sigtimedwait_time64".into(),
			"sigaction".into(),
			"sigaltstack".into(),
			"signal".into(),
			"signalfd".into(),
			"signalfd4".into(),
			"sigpending".into(),
			"sigprocmask".into(),
			"sigsuspend".into(),
		],
		timer: vec![
			"alarm".into(),
			"getitimer".into(),
			"setitimer".into(),
			"timer_create".into(),
			"timer_delete".into(),
			"timer_getoverrun".into(),
			"timer_gettime".into(),
			"timer_gettime64".into(),
			"timer_settime".into(),
			"timer_settime64".into(),
			"timerfd_create".into(),
			"timerfd_gettime".into(),
			"timerfd_gettime64".into(),
			"timerfd_settime".into(),
			"timerfd_settime64".into(),
			"times".into(),
		],
		other: vec![
			"arch_prctl".into(),
			"brk".into(),
			"cacheflush".into(),
			"clock_getres".into(),
			"clock_getres_time64".into(),
			"clock_gettime".into(),
			"clock_gettime64".into(),
			"clock_nanosleep".into(),
			"clock_nanosleep_time64".into(),
			"exit".into(),
			"exit_group".into(),
			"futex".into(),
			"futex_time64".into(),
			"futex_waitv".into(),
			"get_robust_list".into(),
			"get_thread_area".into(),
			"getegid".into(),
			"getegid32".into(),
			"geteuid".into(),
			"geteuid32".into(),
			"getgid".into(),
			"getgid32".into(),
			"getgroups".into(),
			"getgroups32".into(),
			"getpgid".into(),
			"getpgrp".into(),
			"getpid".into(),
			"getppid".into(),
			"getrandom".into(),
			"getresgid".into(),
			"getresgid32".into(),
			"getresuid".into(),
			"getresuid32".into(),
			"getrlimit".into(),
			"getsid".into(),
			"gettid".into(),
			"gettimeofday".into(),
			"getuid".into(),
			"getuid32".into(),
			"lsm_get_self_attr".into(),
			"lsm_list_modules".into(),
			"membarrier".into(),
			"mmap".into(),
			"mmap2".into(),
			"mprotect".into(),
			"mseal".into(),
			"munmap".into(),
			"nanosleep".into(),
			"pause".into(),
			"prlimit64".into(),
			"restart_syscall".into(),
			"riscv_flush_icache".into(),
			"riscv_hwprobe".into(),
			"rseq".into(),
			"rt_sigreturn".into(),
			"sched_getaffinity".into(),
			"sched_yield".into(),
			"set_robust_list".into(),
			"set_thread_area".into(),
			"set_tid_address".into(),
			"set_tls".into(),
			"sigreturn".into(),
			"time".into(),
			"ugetrlimit".into(),
			"uretprobe".into(),
			"arm_fadvise64_64".into(),
			"capget".into(),
			"copy_file_range".into(),
			"fadvise64".into(),
			"fadvise64_64".into(),
			"flock".into(),
			"get_mempolicy".into(),
			"getcpu".into(),
			"getpriority".into(),
			"ioctl".into(),
			"ioprio_get".into(),
			"madvise".into(),
			"mremap".into(),
			"name_to_handle_at".into(),
			"oldolduname".into(),
			"olduname".into(),
			"personality".into(),
			"readahead".into(),
			"readdir".into(),
			"remap_file_pages".into(),
			"sched_get_priority_max".into(),
			"sched_get_priority_min".into(),
			"sched_getattr".into(),
			"sched_getparam".into(),
			"sched_getscheduler".into(),
			"sched_rr_get_interval".into(),
			"sched_rr_get_interval_time64".into(),
			"sched_yield".into(),
			"sendfile".into(),
			"sendfile64".into(),

			"setpgid".into(),
			"setsid".into(),
			"splice".into(),
			"sysinfo".into(),
			"tee".into(),
			"umask".into(),
			"uname".into(),
			"userfaultfd".into(),
			"vmsplice".into(),
		],
	};

	let allowed_syscall_group = vec![
		syscall_by_names.async_io,
		syscall_by_names.basic_io,
		syscall_by_names.fs_op,
		syscall_by_names.io_ev,
		syscall_by_names.ipc,
		syscall_by_names.memlock,
		syscall_by_names.network,
		syscall_by_names.pkey,
		syscall_by_names.resources,
		syscall_by_names.sync,
		syscall_by_names.process,
		syscall_by_names.signal,
		syscall_by_names.timer,
		syscall_by_names.other,
	];
	let denied_syscall_group: Vec<Vec<String>> = vec![
		syscall_by_names.clock,
		syscall_by_names.module,
		syscall_by_names.obsolete,
		syscall_by_names.chown,
		syscall_by_names.raw_io,
		syscall_by_names.reboot,
		syscall_by_names.swap,
		syscall_by_names.setuid,
	];
	let debug_syscall_group: Vec<Vec<String>> = vec![
		syscall_by_names.debug,
	];
	let lockdown_syscall_group: Vec<Vec<String>> = vec![
		syscall_by_names.keyring,
		syscall_by_names.mount,
		syscall_by_names.process_notify,
		vec![
			"kcmp".into(),
			"setfsgid".into(),
			"setfsgid32".into(),
			"setfsuid".into(),
			"setfsuid32".into(),
		],
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

	crate::logger::log_sync(
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
			crate::logger::log_sync(
				logtx,
				crate::logger::Loglevel::Debug,
				format!("Could not resolve syscall from name {name}: {e:#?}"));
			None
		}
	}
}
