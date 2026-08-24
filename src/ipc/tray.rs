
/**
	Scan the local bus and attempt to wake all applications
*/
pub async fn wake() -> zbus::fdo::Result<()> {
	let builder = zbus::conn::Builder::session()
		?;

	let conn = builder
		.build()
		.await
		?;

	let dbus_proxy = zbus::fdo::DBusProxy::new(&conn)
		.await
		?;

	let names = dbus_proxy.list_names().await?;

	{
		let name = match zbus::names::BusName::try_from("org.kde.StatusNotifierWatcher") {
			Ok(v)	=> v,
			Err(e)	=> {
				return Err(
					zbus::fdo::Error::Failed(
						format!("Could not convert bus name: {e:#?}"),
					)
				);
			}
		};

		match dbus_proxy.name_has_owner(name).await? {
			true	=> {}
			false	=> {
				return Err(
					zbus::fdo::Error::NameHasNoOwner(
						"StatusNotifierWatcher has no owner".to_string(),
					)
				);
			}
		}
	};

	for name in names {
		match wake_name(&conn, &name).await {
			Ok(_)	=> {
				#[cfg(debug_assertions)]
				crate::logger::log_debug(
					format!("Successfully woke {name} up")
				);
			}
			Err(e)	=> {
				match e {
					zbus::Error::InterfaceNotFound	=> {}
					zbus::Error::MethodError(_,_,_)	=> {}
					_				=> {
						crate::logger::log_warn(
							format!("Could not activate name {name}: {e:#?}")
						);
					}
				}
			}
		}
	};
	Ok(())
}

async fn wake_name(conn: &zbus::Connection, name: &str) -> zbus::Result<()> {
	let proxy = StatusNotifierItemProxy::new(conn, name)
		.await
		?;
	proxy
		.activate(1, 18)
		.await
}


#[zbus::proxy(
	interface	= "org.kde.StatusNotifierItem",
	default_path	= "/StatusNotifierItem",
)]
trait StatusNotifierItem {
	#[zbus(name = "Activate")]
	async fn activate(&self, x: i32, y: i32) -> zbus::Result<()>;
}
