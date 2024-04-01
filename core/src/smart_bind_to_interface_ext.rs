use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use reqwest::ClientBuilder;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SmartBindToInterfaceError {
    #[error("interface `{0}` not found")]
    InterfaceNotFound(String),
    #[error("failed to get network interfaces")]
    GetNetworkInterfaces(#[from] network_interface::Error),
}

pub trait SmartBindToInterfaceExt
where
    Self: Sized,
{
    fn smart_bind_to_interface(self, interface: &str) -> Result<Self, SmartBindToInterfaceError>;
    fn optional_smart_bind_to_interface(
        self,
        interface: Option<&str>,
    ) -> Result<Self, SmartBindToInterfaceError> {
        if let Some(interface) = interface {
            self.smart_bind_to_interface(interface)
        } else {
            Ok(self)
        }
    }
}

impl SmartBindToInterfaceExt for ClientBuilder {
    fn smart_bind_to_interface(self, interface: &str) -> Result<Self, SmartBindToInterfaceError> {
        if interface.is_empty() {
            return Ok(self);
        }
        let interface_info = NetworkInterface::show()?
            .into_iter()
            .find(|x| x.name == interface);
        if interface_info.is_none() {
            return Err(SmartBindToInterfaceError::InterfaceNotFound(
                interface.to_owned(),
            ));
        }
        #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
        return Ok(self.interface(interface));
        #[cfg(not(any(target_os = "android", target_os = "fuchsia", target_os = "linux")))]
        {
            let interface_info = interface_info.unwrap();
            let ip_addr = interface_info.addr.iter().find_map(|x| match x {
                // prefer ipv4
                network_interface::Addr::V4(_) => Some(x.ip()),
                _ => None,
            });
            let ip_addr = ip_addr.or_else(|| interface_info.addr.first().map(|x| x.ip()));
            if let Some(ip_addr) = ip_addr {
                Ok(self.local_address(ip_addr))
            } else {
                Err(SmartBindToInterfaceError::InterfaceNotFound(
                    interface.to_owned(),
                ))
            }
        }
    }
}
