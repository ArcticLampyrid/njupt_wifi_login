use anyhow::Result;
use futures::stream::StreamExt;
use netlink_packet_core::NetlinkPayload::InnerMessage;
use netlink_packet_route::{rtnl::address::nlas::Nla::Address, RtnlMessage::NewAddress};
use netlink_sys::{AsyncSocket, SocketAddr};
use rtnetlink::new_connection;
use tokio::sync::mpsc::{self, error::TrySendError};

pub struct NetworkChangedListener {}

impl NetworkChangedListener {
    pub fn new() -> Result<Self> {
        Ok(Self {})
    }

    pub fn listen(&self) -> Result<mpsc::Receiver<()>> {
        let (mut conn, mut _handle, mut messages) = new_connection()?;
        let addr = SocketAddr::new(0, libc::RTMGRP_IPV4_IFADDR.try_into().unwrap());
        conn.socket_mut().socket_mut().bind(&addr)?;
        let (tx, rx) = mpsc::channel(1);
        tokio::spawn(conn);
        tokio::spawn(async move {
            while let Some((message, _)) = messages.next().await {
                if let InnerMessage(NewAddress(_)) = message.payload {
                    println!("new addr!!");
                    if let Err(TrySendError::Closed(_)) = tx.try_send(()) {
                        break;
                    }
                }
            }
            drop(tx);
        });
        Ok(rx)
    }
}

impl Drop for NetworkChangedListener {
    fn drop(&mut self) {}
}
