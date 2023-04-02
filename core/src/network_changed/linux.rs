use anyhow::Result;
use futures::stream::StreamExt;
use netlink_packet_core::NetlinkPayload::InnerMessage;
use netlink_packet_route::{route::Nla::Gateway, RtnlMessage::NewRoute};
use netlink_sys::{AsyncSocket, SocketAddr};
use rtnetlink::new_connection;
use tokio::{sync::mpsc::{self, error::TrySendError}, task::JoinHandle};
use tracing::trace;

#[must_use]
pub struct NetworkChangedListener {
    handle_conn: JoinHandle<()>,
    handle_loop: JoinHandle<()>
}

impl NetworkChangedListener {
    pub fn listen() -> Result<(Self, mpsc::Receiver<()>)> {
        let (mut conn, _, mut messages) = new_connection()?;
        let addr = SocketAddr::new(0, (libc::RTMGRP_IPV4_ROUTE).try_into().unwrap());
        conn.socket_mut().socket_mut().bind(&addr)?;
        let (tx, rx) = mpsc::channel(1);
        let _ = tx.try_send(());
        let handle_conn = tokio::spawn(conn);
        let handle_loop = tokio::spawn(async move {
            while let Some((message, _)) = messages.next().await {
                match message.payload {
                    InnerMessage(NewRoute(message)) => {
                        trace!(?message);
                        for attr in message.nlas {
                            if let Gateway(_) = attr {
                                if let Err(TrySendError::Closed(_)) = tx.try_send(()) {
                                    // rx is dropped
                                    return;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
        Ok((Self {handle_conn, handle_loop}, rx))
    }
}

impl Drop for NetworkChangedListener {
    fn drop(&mut self) {
        self.handle_conn.abort();
        self.handle_loop.abort();
    }
}
