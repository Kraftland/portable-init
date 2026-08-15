use thiserror::Error;

#[derive(Debug, Error)]
pub enum ApplyUclampError {

	#[error("Failed to open uclamp file: {0:#?}")]
	OpenFileErr(std::io::Error),

	#[error("Failed to write uclamp file: {0:#?}")]
	WriteFileErr(std::io::Error),
}

pub async fn apply_uclamp (
	config: std::sync::Arc<crate::envs::ConfigOpts>,
) -> Result<(u32, u32), ApplyUclampError> {
	use tokio::io::AsyncWriteExt;

	let mut max_file = tokio::fs::OpenOptions::new()
		.write(true)
		.append(false)
		.open("/sys/fs/cgroup/cpu.uclamp.max")
		.await
		.map_err(ApplyUclampError::OpenFileErr)
		?;

	max_file
		.write(
			config.uclamp_max.to_string().as_bytes()
		)
		.await
		.map_err(ApplyUclampError::WriteFileErr)
		?;

	let mut min_file = tokio::fs::OpenOptions::new()
		.write(true)
		.append(false)
		.open("/sys/fs/cgroup/cpu.uclamp.min")
		.await
		.map_err(ApplyUclampError::OpenFileErr)
		?;

	min_file
		.write(
			config.uclamp_min.to_string().as_bytes()
		)
		.await
		.map_err(ApplyUclampError::WriteFileErr)
		?;

	Ok((config.uclamp_min, config.uclamp_max))
}
