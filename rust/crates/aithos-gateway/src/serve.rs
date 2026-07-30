//! Async serving plumbing of the gateway binary, moved out of `main.rs`
//! at lot SPL-5 so it is testable out of process: the dual-ingress
//! supervisor ([`serve_gateway`]), the relay plane ([`run_relay_plane`])
//! and the public-TLS lifecycle ([`prepare_public_tls`],
//! [`renew_public_tls`]). The library stays pure at its core; this module
//! is the one place the system clock and OS entropy are supplied to the
//! serving path, exactly as the binary did.

use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use rustls::pki_types::UnixTime;
use tokio::sync::watch;

use crate::config::{GatewayConfig, RelayCertificateConfig, RelayConfig};
use crate::core_bridge::{EntropySource as _, OsEntropy};
use crate::keyholder::Keyholder;
use crate::public_tls::{
    load_private_pem, public_tls_slot, AcmeCertificateManager, AcmeTxtClient, CertificateSource,
    InstantAcmeIssuer, PublicTlsAcceptor, PublicTlsActivator, SecureTlsCache,
};
use crate::relay::{RelayClient, RelayHealth, RelayInputs};
use crate::relay_application::relay_application_channel;

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// UTC RFC 3339 (`YYYY-MM-DDTHH:MM:SSZ`), same construction as the two
/// binaries (civil_from_days per Hinnant).
fn ts(secs: u64) -> String {
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (h, m, s) = (rem / 3_600, (rem % 3_600) / 60, rem % 60);
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if mo <= 2 { y + 1 } else { y };
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

const RELAY_APPLICATION_CAPACITY: usize = 64;
const PUBLIC_TLS_RETRY: Duration = Duration::from_secs(60);
const ACME_RENEWAL_CHECK: Duration = Duration::from_secs(6 * 60 * 60);
const ACME_RENEWAL_RETRY: Duration = Duration::from_secs(5 * 60);

pub struct PublicTlsRuntime {
    pub acceptor: PublicTlsAcceptor,
    pub renewal: Option<tokio::task::JoinHandle<()>>,
}

/// Serve one immutable application router through both ingress paths. Relay
/// setup and reconnect are isolated in their own supervisor: a certificate,
/// DNS or tunnel outage cannot take down the historical direct listener.
pub async fn serve_gateway(
    cfg: &GatewayConfig,
    app: Router,
    identity: Arc<Keyholder>,
    relay_health: RelayHealth,
) -> Result<(), Box<dyn std::error::Error>> {
    let listener = tokio::net::TcpListener::bind(&cfg.listen).await?;
    eprintln!("gateway listening on http://{}/mcp", cfg.listen);

    let Some(relay) = cfg.relay.clone() else {
        axum::serve(listener, app).await?;
        return Ok(());
    };

    let (shutdown_sender, shutdown) = watch::channel(false);
    let relay_task = tokio::spawn(run_relay_plane(
        relay,
        identity,
        app.clone(),
        relay_health,
        shutdown,
    ));

    let direct_result = axum::serve(listener, app).await;
    let _ = shutdown_sender.send(true);
    let _ = relay_task.await;
    direct_result?;
    Ok(())
}

pub async fn run_relay_plane(
    config: RelayConfig,
    identity: Arc<Keyholder>,
    app: Router,
    health: RelayHealth,
    mut shutdown: watch::Receiver<bool>,
) {
    loop {
        if *shutdown.borrow() {
            return;
        }

        let tls = match prepare_public_tls(&config, Arc::clone(&identity), shutdown.clone()).await {
            Ok(tls) => tls,
            Err(_) => {
                eprintln!("relay public TLS unavailable; direct listener remains active");
                if wait_for_relay_retry(&mut shutdown, PUBLIC_TLS_RETRY).await {
                    return;
                }
                continue;
            }
        };
        let relay = match RelayClient::from_system_roots(config.clone()) {
            Ok(relay) => relay,
            Err(_) => {
                if let Some(renewal) = tls.renewal {
                    renewal.abort();
                }
                eprintln!("relay trust roots unavailable; direct listener remains active");
                if wait_for_relay_retry(&mut shutdown, PUBLIC_TLS_RETRY).await {
                    return;
                }
                continue;
            }
        };
        let (ingress, relay_listener) = match relay_application_channel(RELAY_APPLICATION_CAPACITY)
        {
            Ok(channel) => channel,
            Err(_) => return,
        };
        let relay_app = app.clone();
        let router_task = tokio::spawn(async move {
            let _ = axum::serve(relay_listener, relay_app).await;
        });
        let acceptor = tls.acceptor.clone();
        let inputs = relay_inputs();
        let relay_result = relay
            .run(
                Arc::clone(&identity),
                inputs,
                health.clone(),
                shutdown.clone(),
                move |stream| {
                    let ingress = ingress.clone();
                    let acceptor = acceptor.clone();
                    async move {
                        let _ = ingress.accept(&acceptor, stream).await;
                    }
                },
            )
            .await;

        router_task.abort();
        if let Some(renewal) = tls.renewal {
            renewal.abort();
        }
        if *shutdown.borrow() {
            return;
        }
        if relay_result.is_err() {
            eprintln!("relay supervisor unavailable; direct listener remains active");
        }
        if wait_for_relay_retry(&mut shutdown, PUBLIC_TLS_RETRY).await {
            return;
        }
    }
}

pub async fn prepare_public_tls(
    config: &RelayConfig,
    identity: Arc<Keyholder>,
    shutdown: watch::Receiver<bool>,
) -> crate::Result<PublicTlsRuntime> {
    match &config.cert {
        RelayCertificateConfig::Pem {
            cert_file,
            key_file,
        } => {
            let current = load_private_pem(cert_file, key_file, &config.hostname, unix_time_now())?;
            let (_fixed, acceptor) = public_tls_slot(current);
            Ok(PublicTlsRuntime {
                acceptor,
                renewal: None,
            })
        }
        RelayCertificateConfig::AcmeDns01 {
            directory,
            store_url,
            cache_dir,
        } => {
            let cache = SecureTlsCache::open(cache_dir.clone())?;
            let dns = AcmeTxtClient::new(
                store_url,
                identity,
                Arc::new(|| ts(now_secs())),
                Arc::new(relay_nonce),
            )?;
            let issuer = InstantAcmeIssuer::new(directory.clone(), dns, cache.clone());
            let manager = Arc::new(AcmeCertificateManager::new(cache, issuer));
            let lease = manager.ensure(&config.hostname, unix_time_now()).await?;
            let (activator, acceptor) = public_tls_slot(lease.config);
            let hostname = config.hostname.clone();
            let renewal = tokio::spawn(renew_public_tls(manager, activator, hostname, shutdown));
            Ok(PublicTlsRuntime {
                acceptor,
                renewal: Some(renewal),
            })
        }
    }
}

pub async fn renew_public_tls(
    manager: Arc<AcmeCertificateManager<InstantAcmeIssuer>>,
    activator: PublicTlsActivator,
    hostname: String,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut delay = ACME_RENEWAL_CHECK;
    loop {
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            changed = shutdown.changed() => {
                if changed.is_err() || *shutdown.borrow() {
                    return;
                }
                continue;
            }
        }
        match manager.ensure(&hostname, unix_time_now()).await {
            Ok(lease) => {
                delay = if lease.source == CertificateSource::RetainedAfterRenewalFailure {
                    ACME_RENEWAL_RETRY
                } else {
                    ACME_RENEWAL_CHECK
                };
                activator.replace(lease.config);
            }
            Err(_) => delay = ACME_RENEWAL_RETRY,
        }
    }
}

async fn wait_for_relay_retry(shutdown: &mut watch::Receiver<bool>, delay: Duration) -> bool {
    tokio::select! {
        _ = tokio::time::sleep(delay) => false,
        changed = shutdown.changed() => changed.is_err() || *shutdown.borrow(),
    }
}

fn relay_inputs() -> RelayInputs {
    RelayInputs {
        clock: Arc::new(|| ts(now_secs())),
        nonce: Arc::new(relay_nonce),
        jitter: Arc::new(relay_jitter),
    }
}

fn relay_nonce() -> String {
    let mut entropy = OsEntropy;
    hex::encode(entropy.e16())
}

fn relay_jitter() -> u64 {
    let mut entropy = OsEntropy;
    let sample = entropy.e16();
    u64::from_le_bytes(sample[..8].try_into().expect("fixed entropy width"))
}

fn unix_time_now() -> UnixTime {
    UnixTime::since_unix_epoch(Duration::from_secs(now_secs()))
}
