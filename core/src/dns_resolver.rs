use hyper::client::connect::dns::Name;
use trust_dns_resolver::config::{ResolverConfig, ResolverOpts};
use trust_dns_resolver::error::ResolveError;
use trust_dns_resolver::{lookup_ip::LookupIpIntoIter, TokioAsyncResolver};

use reqwest::dns::{Addrs, Resolve, Resolving};
use std::net::SocketAddr;
use std::sync::Arc;

type SharedResolver = Arc<TokioAsyncResolver>;
type SharedFallback = Arc<Box<dyn Fn(&Name) -> Option<Addrs> + Send + Sync>>;

#[derive(Clone)]
pub struct CustomTrustDnsResolver {
    shared: SharedResolver,
    fallback: SharedFallback,
}

struct SocketAddrs {
    iter: LookupIpIntoIter,
}

impl CustomTrustDnsResolver {
    pub fn new<F>(
        config: ResolverConfig,
        options: ResolverOpts,
        fallback: F,
    ) -> Result<CustomTrustDnsResolver, ResolveError>
    where
        F: Fn(&Name) -> Option<Addrs> + Send + Sync + 'static,
    {
        Ok(CustomTrustDnsResolver {
            shared: Arc::new(TokioAsyncResolver::tokio(config, options)),
            fallback: Arc::new(Box::new(fallback)),
        })
    }
}

impl Resolve for CustomTrustDnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let shared = self.shared.clone();
        let fallback = self.fallback.clone();
        Box::pin(async move {
            match shared.lookup_ip(name.as_str()).await {
                Ok(lookup) => {
                    let addrs: Addrs = Box::new(SocketAddrs {
                        iter: lookup.into_iter(),
                    });
                    Ok(addrs)
                }
                Err(err) => match fallback(&name) {
                    Some(addrs) => {
                        log::error!(
                            "Fallback addrs for {} is used due to {}",
                            name.as_str(),
                            err
                        );
                        Ok(addrs)
                    }
                    None => Err(err.into()),
                },
            }
        })
    }
}

impl Iterator for SocketAddrs {
    type Item = SocketAddr;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next().map(|ip_addr| SocketAddr::new(ip_addr, 0))
    }
}
