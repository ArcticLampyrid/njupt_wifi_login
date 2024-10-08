use futures_util::Future;
use hickory_proto::iocompat::AsyncIoTokioAsStd;
use hickory_resolver::{name_server::RuntimeProvider, proto::TokioTime, TokioHandle};
use network_interface::{NetworkInterface, NetworkInterfaceConfig};
use std::{
    net::{IpAddr, SocketAddr},
    pin::Pin,
};
use tokio::{
    io,
    net::{TcpSocket as TokioTcpSocket, TcpStream as TokioTcpStream, UdpSocket as TokioUdpSocket},
};

#[derive(Clone)]
pub struct BindableTokioRuntimeProvider {
    handle: TokioHandle,
    interface: Option<String>,
}

impl BindableTokioRuntimeProvider {
    pub fn new(interface: Option<String>) -> Self {
        Self {
            handle: TokioHandle::default(),
            interface,
        }
    }
}

enum AddressFamilyPreference {
    V4,
    V6,
}

impl AddressFamilyPreference {
    pub fn for_addr(addr: IpAddr) -> Self {
        match addr {
            IpAddr::V4(_) => AddressFamilyPreference::V4,
            IpAddr::V6(_) => AddressFamilyPreference::V6,
        }
    }
    pub fn is_preferred(&self, addr: IpAddr) -> bool {
        match self {
            AddressFamilyPreference::V4 => addr.is_ipv4(),
            AddressFamilyPreference::V6 => addr.is_ipv6(),
        }
    }
}

fn get_bind_addr(interface: &str, preference: AddressFamilyPreference) -> io::Result<SocketAddr> {
    let interfaces = match NetworkInterface::show() {
        Ok(interfaces) => interfaces,
        Err(e) => return Err(io::Error::new(io::ErrorKind::Other, e)),
    };
    let interface_info = interfaces.into_iter().find(|x| x.name == interface);
    if interface_info.is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "interface not found",
        ));
    }
    let interface_info = interface_info.unwrap();
    let ip_addr = interface_info.addr.iter().find_map(|x| {
        if preference.is_preferred(x.ip()) {
            Some(x.ip())
        } else {
            None
        }
    });
    let ip_addr = ip_addr.or_else(|| interface_info.addr.first().map(|x| x.ip()));
    if let Some(ip_addr) = ip_addr {
        Ok(SocketAddr::new(ip_addr, 0))
    } else {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            "interface not found",
        ))
    }
}

impl RuntimeProvider for BindableTokioRuntimeProvider {
    type Handle = TokioHandle;
    type Timer = TokioTime;
    type Udp = TokioUdpSocket;
    type Tcp = AsyncIoTokioAsStd<TokioTcpStream>;

    fn create_handle(&self) -> Self::Handle {
        self.handle.clone()
    }

    fn connect_tcp(
        &self,
        server_addr: SocketAddr,
    ) -> Pin<Box<dyn Send + Future<Output = io::Result<Self::Tcp>>>> {
        let interface = self.interface.clone();
        Box::pin(async move {
            let socket = match server_addr {
                SocketAddr::V4(_) => TokioTcpSocket::new_v4(),
                SocketAddr::V6(_) => TokioTcpSocket::new_v6(),
            }?;
            if cfg!(not(any(
                target_os = "android",
                target_os = "fuchsia",
                target_os = "linux"
            ))) {
                if let Some(interface) = interface.as_ref() {
                    let bind_addr = get_bind_addr(
                        interface,
                        AddressFamilyPreference::for_addr(server_addr.ip()),
                    )?;
                    socket.bind(bind_addr)?;
                }
            }
            #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
            socket.bind_device(interface.as_ref().map(|iface| iface.as_bytes()))?;
            socket.connect(server_addr).await.map(AsyncIoTokioAsStd)
        })
    }

    fn bind_udp(
        &self,
        local_addr: SocketAddr,
        server_addr: SocketAddr,
    ) -> Pin<Box<dyn Send + Future<Output = io::Result<Self::Udp>>>> {
        let interface = self.interface.clone();
        Box::pin(async move {
            let bind_to = if cfg!(any(
                target_os = "android",
                target_os = "fuchsia",
                target_os = "linux"
            )) {
                local_addr
            } else {
                match interface.as_ref() {
                    Some(interface) => get_bind_addr(
                        interface,
                        AddressFamilyPreference::for_addr(server_addr.ip()),
                    )?,
                    None => local_addr,
                }
            };
            let socket = TokioUdpSocket::bind(bind_to).await?;
            #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
            socket.bind_device(interface.as_ref().map(|iface| iface.as_bytes()))?;
            Ok(socket)
        })
    }
}
