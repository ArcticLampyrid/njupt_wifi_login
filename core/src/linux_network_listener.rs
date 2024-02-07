#![cfg(target_os = "linux")]
use futures_util::stream::StreamExt;
use log::trace;
use netlink_packet_core::NetlinkPayload;
use netlink_packet_route::{route::RouteAttribute, RouteNetlinkMessage};
use netlink_sys::{AsyncSocket, SocketAddr};
use rtnetlink::new_connection;
use tokio::task::JoinHandle;
#[must_use]
pub struct LinuxNetworkListenerHandle {
    handle_conn: JoinHandle<()>,
    handle_polling: JoinHandle<()>,
}
impl LinuxNetworkListenerHandle {
    pub fn register<F>(func: F) -> Result<Self, Box<dyn std::error::Error + Sync + Send>>
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
                    for attr in &message.attributes {
                        if let RouteAttribute::Gateway(_) = attr {
                            trace!("Gateway changed: {:?}", message);
                            func();
                            break;
                        }
                    }
                }
            }
        });

        Ok(Self {
            handle_conn,
            handle_polling,
        })
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
