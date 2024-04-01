#![cfg(target_os = "linux")]
use futures_util::stream::StreamExt;
use log::trace;
use netlink_packet_core::NetlinkPayload;
use netlink_packet_route::{
    route::{RouteAttribute, RouteMessage},
    RouteNetlinkMessage,
};
use netlink_sys::{AsyncSocket, SocketAddr};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use rtnetlink::new_connection;
use tokio::task::JoinHandle;
#[must_use]
pub struct LinuxNetworkListenerHandle {
    handle_conn: JoinHandle<()>,
    handle_polling: JoinHandle<()>,
}
impl LinuxNetworkListenerHandle {
    pub fn register<F>(
        func: F,
        interface: Option<String>,
    ) -> Result<Self, Box<dyn std::error::Error + Sync + Send>>
    where
        F: Fn() + Send + 'static,
    {
        let (mut conn, mut _handle, mut messages) = new_connection()?;

        let addr = SocketAddr::new(0, libc::RTMGRP_IPV4_ROUTE as u32);
        conn.socket_mut().socket_mut().bind(&addr)?;
        let handle_conn = tokio::spawn(conn);
        let handle_polling = tokio::spawn(async move {
            while let Some((message, _)) = messages.next().await {
                if let NetlinkPayload::InnerMessage(RouteNetlinkMessage::NewRoute(message)) =
                    message.payload
                {
                    let gateway = message.attributes.iter().find_map(|attr| {
                        if let RouteAttribute::Gateway(addr) = attr {
                            Some(addr)
                        } else {
                            None
                        }
                    });
                    if gateway.is_some() {
                        if !Self::match_interface(&message, interface.as_deref()) {
                            trace!("Skipping due to interface mismatch: {:?}", message);
                            continue;
                        }
                        trace!("Gateway changed: {:?}", message);
                        func();
                    }
                }
            }
        });

        Ok(Self {
            handle_conn,
            handle_polling,
        })
    }

    fn match_interface(message: &RouteMessage, interface: Option<&str>) -> bool {
        if interface.is_none() {
            return true;
        }
        let interface_name = interface.unwrap();
        if interface_name.is_empty() {
            return true;
        }
        let oif = message.attributes.iter().find_map(|attr| {
            if let RouteAttribute::Oif(oif) = attr {
                Some(oif)
            } else {
                None
            }
        });
        if let Some(oif) = oif {
            let interface_info = NetworkInterface::show()
                .ok()
                .and_then(|x| x.into_iter().find(|x| x.index == *oif));
            interface_info.map_or(false, |x| x.name == interface_name)
        } else {
            false
        }
    }

    pub fn abort(&self) {
        self.handle_conn.abort();
        self.handle_polling.abort();
    }

    pub async fn join(self) {
        let _ = self.handle_conn.await;
        let _ = self.handle_polling.await;
    }
}
