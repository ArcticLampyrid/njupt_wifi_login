use super::runtime::BindableTokioRuntimeProvider;
use display_error_chain::ErrorChainExt;
use hickory_resolver::config::{ResolverConfig, ResolverOpts};
use hickory_resolver::error::ResolveError;
use hickory_resolver::lookup_ip::LookupIpIntoIter;
use hickory_resolver::name_server::GenericConnector;
use hickory_resolver::AsyncResolver;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::SocketAddr;
use std::sync::Arc;

type SharedResolver = Arc<AsyncResolver<GenericConnector<BindableTokioRuntimeProvider>>>;
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
        interface: Option<String>,
        config: ResolverConfig,
        options: ResolverOpts,
        fallback: F,
    ) -> Result<CustomTrustDnsResolver, ResolveError>
    where
        F: Fn(&Name) -> Option<Addrs> + Send + Sync + 'static,
    {
        let connector = GenericConnector::new(BindableTokioRuntimeProvider::new(interface));
        Ok(CustomTrustDnsResolver {
            shared: Arc::new(AsyncResolver::new(config, options, connector)),
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
                            err.chain()
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
