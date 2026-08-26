use thiserror::Error;

mod list;
mod unotify;
mod filter;

/**
	Compile and load the secure computing filter.

	This is a unified version of previous APIs, and it works in an async fashion.
*/
pub async fn load(
	config:		std::sync::Arc<crate::envs::ConfigOpts>,
	cancel_token:	tokio_util::sync::CancellationToken,
) -> Result<(), SeccompError> {
	let list = list::compile_syscall_list()
		.map_err(SeccompError::CompileListError)
		?;

	let seccomp_filter = filter::compile_filter(
		config,
		&list,
	)
		.await
		?;

	let unotify_fd = filter::load_seccomp_filter(seccomp_filter)
		?;

	std::thread::spawn(
		move || {
			unotify::process_seccomp_unotify(unotify_fd, cancel_token);
		},
	);

	Ok(())
}

#[derive(Error, Debug)]
pub enum SeccompError {
	#[error("Could not compile seccomp syscall list: {0:#?}")]
	CompileListError(SyscallCompileError),

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


