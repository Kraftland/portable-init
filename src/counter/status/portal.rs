/**
	This status subsystem presents user with PID tracking notification inside the
		Background Apps UI
*/
pub struct PortalStatus {
	pub bus:	zbus::Connection,
}

impl super::UpdateStatus for PortalStatus {
	async fn update(&self, status: &super::SandboxStatus) -> Result<(), Self::StatusError> {
		let proxy = PortalProxy::new(&self.bus)
			.await
			.map_err(PortalError::ProxyError)
			?;

		let content = status.to_string();

		let options = {
			use std::collections::HashMap;
			use zbus::zvariant::OwnedValue;

			let mut map = HashMap::new();

			map.insert(
				"message".to_string(),
				OwnedValue::try_from(
					zbus::zvariant::Value::Str(content.into())
				)
					.map_err(PortalError::VariantError)
					?,
			);

			map
		};

		proxy
			.set_status(options)
			.await
			.map_err(PortalError::PortalError)
	}

	type StatusError = PortalError;
}

#[derive(Debug, thiserror::Error)]
pub enum PortalError {
	#[error("Create proxy error: {0:#?}")]
	ProxyError(zbus::Error),

	#[error("Portal error: {0:#?}")]
	PortalError(zbus::Error),

	#[error("Variant error: {0:#?}")]
	VariantError(zbus::zvariant::Error),
}

#[zbus::proxy(
	interface	= "org.freedesktop.portal.Background",
	default_service	= "org.freedesktop.portal.Desktop",
	default_path	= "/org/freedesktop/portal/desktop",
)]
trait Portal {
	async fn set_status(&self, opt: std::collections::HashMap<String, zbus::zvariant::OwnedValue>) -> zbus::Result<()>;
}
