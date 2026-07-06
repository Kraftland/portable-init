use thiserror::Error;

#[derive(Error, Debug)]
pub enum InhibitError {
	#[error("Unable to create Inhibit proxy: {0:#?}")]
	CreateProxyError(ashpd::Error),
	#[error("Unable to request inhibition: {0:#?}")]
	CallInhibitError(ashpd::Error),
}

pub async fn inhibit_suspend(
	cancel_token:	tokio_util::sync::CancellationToken,
) -> Result<(), InhibitError> {
	let proxy = match ashpd::desktop::inhibit::InhibitProxy::new().await {
		Ok(v)	=> v,
		Err(e)	=> {
			return Err(
				InhibitError::CreateProxyError(e)
			)
		}
	};

	let inhibit_options = ashpd::desktop::inhibit::InhibitOptions::default()
		.set_reason(Some("Package requested inhibition"));

	match proxy.inhibit(
		None,
		ashpd::desktop::inhibit::InhibitFlags::Suspend.into(),
		inhibit_options,
	).await {
		Ok(v)	=> {
			tokio::spawn(async move {
				crate::logger::log_debug(
					format!("Inhibited suspend on package request"),
				);
				cancel_token.cancelled().await;
				match v.close().await {
					Ok(_)	=> {}
					Err(e)	=> {
						crate::logger::log_warn(
							format!("Could not end inhibit session: {e:#?}")
						);
					}
				};
			});
			Ok(())
		}
		Err(e)	=> {
			return Err(
				InhibitError::CallInhibitError(e)
			)
		}
	}
}
