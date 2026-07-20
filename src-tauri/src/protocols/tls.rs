use std::sync::{Arc, OnceLock};

use rustls::{ClientConfig, RootCertStore};
use tokio_rustls::TlsConnector;

use crate::core::{CommandError, CommandResult};

#[derive(Clone, Copy, Debug)]
enum NativeTlsConfigError {
    CertificatesUnavailable,
}

static NATIVE_TLS_CONFIG: OnceLock<Result<Arc<ClientConfig>, NativeTlsConfigError>> =
    OnceLock::new();

pub(crate) fn native_tls_connector(unavailable_code: &str) -> CommandResult<TlsConnector> {
    cached_config(&NATIVE_TLS_CONFIG, load_native_tls_config)
        .map(TlsConnector::from)
        .map_err(|_| CommandError::new(unavailable_code))
}

fn cached_config(
    cache: &OnceLock<Result<Arc<ClientConfig>, NativeTlsConfigError>>,
    loader: impl FnOnce() -> Result<Arc<ClientConfig>, NativeTlsConfigError>,
) -> Result<Arc<ClientConfig>, NativeTlsConfigError> {
    cache.get_or_init(loader).clone()
}

fn load_native_tls_config() -> Result<Arc<ClientConfig>, NativeTlsConfigError> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        return Err(NativeTlsConfigError::CertificatesUnavailable);
    }
    let mut roots = RootCertStore::empty();
    for certificate in native.certs {
        let _ = roots.add(certificate);
    }
    if roots.is_empty() {
        return Err(NativeTlsConfigError::CertificatesUnavailable);
    }
    Ok(Arc::new(
        ClientConfig::builder()
            .with_root_certificates(roots)
            .with_no_client_auth(),
    ))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn connector_configuration_is_initialized_once() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let cache = OnceLock::new();
        let loads = AtomicUsize::new(0);
        let load = || {
            loads.fetch_add(1, Ordering::SeqCst);
            Ok(Arc::new(
                ClientConfig::builder()
                    .with_root_certificates(RootCertStore::empty())
                    .with_no_client_auth(),
            ))
        };

        let first = cached_config(&cache, load).unwrap();
        let second = cached_config(&cache, load).unwrap();

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(loads.load(Ordering::SeqCst), 1);
    }
}
