pub async fn inhibit_suspend(
	cancel_token:	tokio_util::sync::CancellationToken,
) {
	let proxy = match ashpd::desktop::inhibit::InhibitProxy::new().await {
		Ok(v)	=> v,
		Err(e)	=> {
			// return Err(
			// 	InhibitError::CreateProxyError(e)
			// )
			crate::logger::log_warn(
				format!("Could not create inhibit proxy: {e:#?}")
			);
			return;
		}
	};

	let inhibit_options = ashpd::desktop::inhibit::InhibitOptions::default()
		.set_reason(Some("Inhibit requested by Portable configuration"),
	);

	let inhibit_result = Box::pin(proxy.inhibit(
		None,
		ashpd::desktop::inhibit::InhibitFlags::Suspend.into(),
		inhibit_options,
		//ashpd::desktop::inhibit::InhibitOptions::default(),
	)).await;

	match inhibit_result {
		Ok(v)	=> {
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
			// Ok(())
		}
		Err(e)	=> {
			crate::logger::log_warn(
				format!("Could not inhibit suspend: {e:#?}"),
			);
		}
	};
}
