use hyper::client::connect::dns::Name;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};
use trust_dns_resolver::error::ResolveError;
use trust_dns_resolver::{lookup_ip::LookupIpIntoIter, TokioAsyncResolver};

use reqwest::dns::{Addrs, Resolve, Resolving};
use std::net::SocketAddr;
use std::sync::Arc;

type SharedResolver = Arc<TokioAsyncResolver>;

#[derive(Debug, Clone)]
pub struct CustomTrustDnsResolver {
    shared: SharedResolver,
}

struct SocketAddrs {
    iter: LookupIpIntoIter,
}

impl CustomTrustDnsResolver {
    pub fn new(
        config: ResolverConfig,
        options: ResolverOpts,
    ) -> Result<CustomTrustDnsResolver, ResolveError> {
        Ok(CustomTrustDnsResolver {
            shared: Arc::new(TokioAsyncResolver::tokio(config, options)),
        })
    }
}

impl Resolve for CustomTrustDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let shared = self.shared.clone();
        Box::pin(async move {
            let lookup = shared.lookup_ip(name.as_str()).await?;
            let addrs: Addrs = Box::new(SocketAddrs {
                iter: lookup.into_iter(),
            });
            Ok(addrs)
        })
    }
}

impl Iterator for SocketAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ip_addr| SocketAddr::new(ip_addr, 0))
    }
}
