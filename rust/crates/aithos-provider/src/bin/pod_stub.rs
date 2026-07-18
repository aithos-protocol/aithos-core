//! aithos-pod-stub — a stand-in MCP pod for the M2 joignabilité gate
//! (dev/e2e only, behind the `pod-stub` feature; NEVER a provider
//! service).
//!
//! It plays the CLIENT far end of annexe B: it dials **out** to the relay
//! tunnel door (TLS + ALPN `aithos-tunnel/1`), sends the signed B.2
//! registration line, then becomes the **yamux server** on that
//! connection. Each inbound yamux stream is a public browser connection
//! the relay piped in raw (ClientHello first, B.3): the pod terminates
//! **its own** public TLS (its key never leaves here, A3) and answers a
//! minimal HTTP `/healthz`. That proves a real HTTPS request reaches the
//! pod end to end through a blind relay — "un MCP réellement joignable au
//! navigateur".
//!
//! The gateway key is the committed p3 TEST key by default (public,
//! `z6MksPyk…`), so the bundled relay bootstrap maps it to
//! `demo.mcp.aithos.fr`. No client secret is invented; the pod signs with
//! a key it owns.
//!
//! | Variable | Rôle | Défaut |
//! |---|---|---|
//! | `RELAY_HOST` / `RELAY_PORT` | relay public door | `relay.aithos.fr` / `443` |
//! | `RELAY_TUNNEL_NAME` | the relay's own SNI (tunnel door) | `relay.aithos.fr` |
//! | `POD_TENANT` / `POD_HOSTNAME` | this pod's enrolment | `acme` / `demo.mcp.aithos.fr` |
//! | `POD_GATEWAY_SEED_HEX` | 32-byte Ed25519 seed (hex) | p3 test seed `51..51` |
//! | `POD_TLS_CERT` / `POD_TLS_KEY` | public cert+key PEM (a real LE cert) | self-signed + printed CA |
//! | `POD_RELAY_CA` | PEM CA to trust for the relay tunnel door | system roots |
//!
//! **ACME mode (annexe B.5, the CLIENT half):** `POD_ACME=1` makes the
//! pod obtain its own public certificate before dialing the relay — the
//! ACME conversation runs HERE, the DNS-01 TXT is posed through the
//! store's delegated `PUT /acme/txt` (signed by the gateway key, the
//! same one that registers the tunnel — zero new secret), and the
//! private key is generated locally and NEVER leaves this process (A3).
//! This removes the manual-cert stopgap: the demo pod self-provisions
//! `<org>.mcp.aithos.fr`.
//!
//! | Variable | Rôle | Défaut |
//! |---|---|---|
//! | `POD_ACME` | `1` = obtain the public cert via /acme/txt | off |
//! | `POD_ACME_DIRECTORY` | ACME directory URL | Let's Encrypt production |
//! | `POD_ACME_STORE_URL` | the store serving /acme/txt | `https://store.aithos.fr` |
//! | `POD_ACME_STORE_CA` | PEM CA pin for the store TLS (dev) | system roots |
//! | `POD_ACME_CACHE` | cert+key cache directory | `./pod-acme-cache` |
//! | `POD_ACME_CONTACT` | account contact (`mailto:…`) | none |
//! | `POD_ACME_DNS_WAIT_SECS` | wait after posing the TXT | `30` |
//! | `POD_ACME_FORCE` | `1` = ignore the cache (renew now) | off |

use std::sync::Arc;

use aithos_provider::sni::TUNNEL_ALPN;
use aithos_provider::tunnel::{
    registration_line, sign_registration, Registration, RegistrationSignature, TUNNEL_WIRE_VERSION,
};
use ed25519_dalek::SigningKey;
use futures::future::poll_fn;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::compat::{FuturesAsyncReadCompatExt, TokioAsyncReadCompatExt};

fn env_or(name: &str, default: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| default.to_owned())
}

fn now_z() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    aithos_provider::time::render_rfc3339z(secs as i64 * 1000)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    let relay_host = env_or("RELAY_HOST", "relay.aithos.fr");
    let relay_port: u16 = env_or("RELAY_PORT", "443").parse()?;
    let tunnel_name = env_or("RELAY_TUNNEL_NAME", "relay.aithos.fr");
    let tenant = env_or("POD_TENANT", "acme");
    let hostname = env_or("POD_HOSTNAME", "demo.mcp.aithos.fr");
    let seed_hex = env_or("POD_GATEWAY_SEED_HEX", &"51".repeat(32));
    let seed: [u8; 32] = hex::decode(&seed_hex)?
        .try_into()
        .map_err(|_| "POD_GATEWAY_SEED_HEX must be 32 bytes")?;
    let gateway_sk = SigningKey::from_bytes(&seed);
    let gateway_pub =
        aithos_core::wire::ed25519_pub_to_multibase(&gateway_sk.verifying_key().to_bytes());

    // Public-facing TLS: ACME mode self-provisions a real cert through
    // the delegated /acme/txt (the key never leaves this process, A3);
    // else a provided cert; else a self-signed one whose CA we print so
    // the prober can pin it (`curl --cacert`).
    let pod_tls = if std::env::var("POD_ACME").as_deref() == Ok("1") {
        let (chain_pem, key_pem) = acme_obtain_certificate(&hostname, &gateway_sk).await?;
        build_pod_tls_from_pems(chain_pem.as_bytes(), key_pem.as_bytes())?
    } else {
        build_pod_tls(&hostname)?
    };

    // --- dial the relay tunnel door: TLS + ALPN aithos-tunnel/1 ----------
    let relay_ca = std::env::var("POD_RELAY_CA").ok();
    let connector = build_relay_connector(relay_ca.as_deref())?;
    let tcp = tokio::net::TcpStream::connect((relay_host.as_str(), relay_port)).await?;
    // Redline B.3 (M2): TCP keepalive on the pod side of the tunnel too —
    // a dead relay is detected without a FIN, and the NAT mapping of this
    // outbound connection stays warm.
    if let Err(e) = aithos_provider::keepalive::enable_tunnel_keepalive(&tcp) {
        eprintln!("pod-stub: tcp keepalive not set: {e}");
    }
    let server_name = rustls::pki_types::ServerName::try_from(tunnel_name.clone())?;
    let mut tls = connector.connect(server_name, tcp).await?;
    eprintln!("pod-stub: tunnel TLS up to {relay_host}:{relay_port} (SNI {tunnel_name}, ALPN aithos-tunnel/1)");

    // --- B.2 registration line ------------------------------------------
    let reg = Registration {
        version: TUNNEL_WIRE_VERSION.into(),
        tenant: tenant.clone(),
        hostname: hostname.clone(),
        gateway_pub: gateway_pub.clone(),
        at: now_z(),
        nonce: format!("pod-stub-{}", std::process::id()),
        signature: RegistrationSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let line = registration_line(&sign_registration(reg, &gateway_sk));
    tls.write_all(line.as_bytes()).await?;
    tls.flush().await?;

    let mut answer = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        let n = tls.read(&mut byte).await?;
        if n == 0 || byte[0] == b'\n' {
            break;
        }
        answer.push(byte[0]);
    }
    let answer = String::from_utf8_lossy(&answer);
    eprintln!("pod-stub: relay answered {answer}");
    if !answer.contains("\"ok\":true") && !answer.contains("\"ok\": true") {
        return Err(format!("relay refused registration: {answer}").into());
    }
    eprintln!("pod-stub: registered for {hostname} — serving as yamux server");

    // --- yamux server: accept piped public connections ------------------
    let cfg = yamux::Config::default();
    let mut conn = yamux::Connection::new(tls.compat(), cfg, yamux::Mode::Server);

    loop {
        let stream = match poll_fn(|cx| conn.poll_next_inbound(cx)).await {
            Some(Ok(s)) => s,
            Some(Err(e)) => {
                eprintln!("pod-stub: yamux error, tunnel closing: {e}");
                break;
            }
            None => {
                eprintln!("pod-stub: relay sent GoAway / tunnel closed");
                break;
            }
        };
        let acceptor = tokio_rustls::TlsAcceptor::from(pod_tls.clone());
        let host = hostname.clone();
        tokio::spawn(async move {
            if let Err(e) = serve_public_stream(stream, acceptor, host).await {
                eprintln!("pod-stub: public stream ended: {e}");
            }
        });
    }
    Ok(())
}

/// Terminate the public TLS on a piped stream (the pod's own cert) and
/// answer a minimal HTTP request — the proof the request reached the pod.
async fn serve_public_stream(
    stream: yamux::Stream,
    acceptor: tokio_rustls::TlsAcceptor,
    hostname: String,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut tls = acceptor.accept(stream.compat()).await?;
    let mut buf = [0u8; 2048];
    let n = tls.read(&mut buf).await?;
    let req = String::from_utf8_lossy(&buf[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let body = format!(
        "{{\"pod\":\"{hostname}\",\"path\":\"{path}\",\"served_by\":\"aithos-pod-stub\",\"reachable\":true}}"
    );
    let resp = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        body.len(),
        body
    );
    tls.write_all(resp.as_bytes()).await?;
    tls.flush().await?;
    let _ = tls.shutdown().await;
    Ok(())
}

// ===================================================================
// ACME mode — the CLIENT half of annexe B.5 (dev/e2e only)
// ===================================================================

/// Obtain (or reuse from cache) the pod's public certificate through the
/// delegated DNS-01: the ACME order runs here, the challenge TXT is
/// posed via the store's signed `PUT /acme/txt`, and the private key is
/// generated locally — it exists nowhere else (A3).
async fn acme_obtain_certificate(
    hostname: &str,
    gateway_sk: &SigningKey,
) -> Result<(String, String), Box<dyn std::error::Error>> {
    let cache_dir = env_or("POD_ACME_CACHE", "./pod-acme-cache");
    let cert_path = format!("{cache_dir}/{hostname}.cert.pem");
    let key_path = format!("{cache_dir}/{hostname}.key.pem");
    let force = std::env::var("POD_ACME_FORCE").as_deref() == Ok("1");
    if !force {
        if let (Ok(chain), Ok(key)) = (
            std::fs::read_to_string(&cert_path),
            std::fs::read_to_string(&key_path),
        ) {
            eprintln!("pod-stub: acme cache hit for {hostname} ({cert_path})");
            return Ok((chain, key));
        }
    }

    let directory = env_or(
        "POD_ACME_DIRECTORY",
        instant_acme::LetsEncrypt::Production.url(),
    );
    let contact_env = std::env::var("POD_ACME_CONTACT").ok();
    let contact: Vec<&str> = contact_env.as_deref().into_iter().collect();
    eprintln!("pod-stub: acme order for {hostname} against {directory}");
    let (account, _credentials) = instant_acme::Account::create(
        &instant_acme::NewAccount {
            contact: &contact,
            terms_of_service_agreed: true,
            only_return_existing: false,
        },
        &directory,
        None,
    )
    .await?;

    let identifier = instant_acme::Identifier::Dns(hostname.to_owned());
    let mut order = account
        .new_order(&instant_acme::NewOrder {
            identifiers: &[identifier],
        })
        .await?;

    // One identifier → one authorization; pose its dns-01 TXT through
    // the store, then tell the CA to validate.
    let dns_wait: u64 = env_or("POD_ACME_DNS_WAIT_SECS", "30").parse()?;
    let mut posed: Vec<String> = Vec::new();
    let authorizations = order.authorizations().await?;
    for authz in &authorizations {
        match authz.status {
            instant_acme::AuthorizationStatus::Valid => continue,
            instant_acme::AuthorizationStatus::Pending => {}
            other => return Err(format!("authorization is {other:?}").into()),
        }
        let challenge = authz
            .challenges
            .iter()
            .find(|c| c.r#type == instant_acme::ChallengeType::Dns01)
            .ok_or("the CA offered no dns-01 challenge")?;
        let value = order.key_authorization(challenge).dns_value();
        eprintln!("pod-stub: posing TXT _acme-challenge.{hostname} via the store (B.5)");
        let status = store_acme_txt("PUT", hostname, &value, gateway_sk).await?;
        if status != 204 {
            return Err(format!("store refused the challenge TXT: HTTP {status}").into());
        }
        posed.push(value);
        eprintln!("pod-stub: TXT posed; waiting {dns_wait}s for propagation");
        tokio::time::sleep(std::time::Duration::from_secs(dns_wait)).await;
        order.set_challenge_ready(&challenge.url).await?;
    }

    // Poll the order to Ready (the CA validates the TXT), then finalize
    // with a CSR over a key generated HERE.
    let mut tries = 0u32;
    let state = loop {
        let state = order.refresh().await?;
        match state.status {
            instant_acme::OrderStatus::Ready | instant_acme::OrderStatus::Valid => break state,
            instant_acme::OrderStatus::Invalid => {
                cleanup_txt(hostname, &posed, gateway_sk).await;
                return Err("acme order went invalid (challenge failed)".into());
            }
            _ if tries > 30 => {
                cleanup_txt(hostname, &posed, gateway_sk).await;
                return Err("acme order never became ready".into());
            }
            _ => {
                tries += 1;
                tokio::time::sleep(std::time::Duration::from_secs(2 + u64::from(tries) / 4)).await;
            }
        }
    };
    let _ = state;

    let mut params = rcgen::CertificateParams::new(vec![hostname.to_owned()])?;
    params.distinguished_name = rcgen::DistinguishedName::new();
    let key_pair = rcgen::KeyPair::generate()?;
    let csr = params.serialize_request(&key_pair)?;
    order.finalize(csr.der()).await?;

    let chain_pem = loop {
        if let Some(chain) = order.certificate().await? {
            break chain;
        }
        tries += 1;
        if tries > 60 {
            cleanup_txt(hostname, &posed, gateway_sk).await;
            return Err("certificate never issued".into());
        }
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    };
    let key_pem = key_pair.serialize_pem();

    // Retire the challenge (the server purge is the backstop), cache,
    // done. The KEY stays in this cache and this process — nowhere else.
    cleanup_txt(hostname, &posed, gateway_sk).await;
    std::fs::create_dir_all(&cache_dir)?;
    std::fs::write(&cert_path, &chain_pem)?;
    std::fs::write(&key_path, &key_pem)?;
    eprintln!("pod-stub: certificate issued for {hostname}, cached at {cert_path}");
    Ok((chain_pem, key_pem))
}

/// Best-effort DELETE of the posed challenge value(s) — the store's
/// 10-minute purge is the backstop, so failure here only logs.
async fn cleanup_txt(hostname: &str, values: &[String], gateway_sk: &SigningKey) {
    for value in values {
        match store_acme_txt("DELETE", hostname, value, gateway_sk).await {
            Ok(204) => {}
            Ok(status) => eprintln!("pod-stub: challenge cleanup answered HTTP {status}"),
            Err(e) => eprintln!("pod-stub: challenge cleanup failed: {e}"),
        }
    }
}

/// One signed `/acme/txt` call (annexe B.5): envelope A.2 with
/// `key = gateway_pub`, `mandate: []`, over HTTPS to the store. A
/// deliberately minimal HTTP/1.1 client — what is signed is exactly what
/// is sent, no client-library ambiguity.
async fn store_acme_txt(
    method: &str,
    hostname: &str,
    value: &str,
    gateway_sk: &SigningKey,
) -> Result<u16, Box<dyn std::error::Error>> {
    use aithos_provider::envelope::{header_value, sign_envelope, Envelope, EnvelopeSignature};

    let store_url = env_or("POD_ACME_STORE_URL", "https://store.aithos.fr");
    let rest = store_url
        .strip_prefix("https://")
        .ok_or("POD_ACME_STORE_URL must be https://…")?;
    let authority_raw = rest.split('/').next().unwrap_or(rest).to_ascii_lowercase();
    let (host, port) = match authority_raw.rsplit_once(':') {
        Some((h, p)) => (h.to_owned(), p.parse::<u16>()?),
        None => (authority_raw.clone(), 443u16),
    };
    // The envelope's authority: lowercase, default port stripped (A.1).
    let authority = if port == 443 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };

    let body = serde_jcs::to_string(&serde_json::json!({
        "hostname": hostname,
        "value": value,
    }))?;
    let nonce = format!(
        "pod-acme-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_millis()
    );
    let envelope = Envelope {
        v: 1,
        host: authority.clone(),
        method: method.to_owned(),
        path: "/acme/txt".to_owned(),
        body_b3: blake3::hash(body.as_bytes()).to_hex().to_string(),
        at: now_z(),
        nonce,
        mandate: vec![],
        key: aithos_core::wire::ed25519_pub_to_multibase(&gateway_sk.verifying_key().to_bytes()),
        signature: EnvelopeSignature {
            alg: "ed25519".into(),
            value: String::new(),
        },
    };
    let envelope = sign_envelope(envelope, gateway_sk).map_err(|e| format!("{e:?}"))?;
    let auth = header_value(&envelope).map_err(|e| format!("{e:?}"))?;

    let store_ca = std::env::var("POD_ACME_STORE_CA").ok();
    let connector = build_https_connector(store_ca.as_deref())?;
    let tcp = tokio::net::TcpStream::connect((host.as_str(), port)).await?;
    let server_name = rustls::pki_types::ServerName::try_from(host.clone())?;
    let mut tls = connector.connect(server_name, tcp).await?;

    let request = format!(
        "{method} /acme/txt HTTP/1.1\r\nhost: {authority}\r\nx-aithos-auth: {auth}\r\n\
         content-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
        body.len(),
    );
    tls.write_all(request.as_bytes()).await?;
    tls.flush().await?;
    let mut raw = Vec::new();
    let _ = tls.read_to_end(&mut raw).await;
    let text = String::from_utf8_lossy(&raw);
    let status: u16 = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| {
            format!(
                "unparsable store response: {}",
                text.chars().take(120).collect::<String>()
            )
        })?;
    Ok(status)
}

/// Plain HTTPS client config (no tunnel ALPN): system roots, or a pinned
/// PEM CA (dev store).
fn build_https_connector(
    ca_pem: Option<&str>,
) -> Result<tokio_rustls::TlsConnector, Box<dyn std::error::Error>> {
    let mut roots = rustls::RootCertStore::empty();
    if let Some(pem) = ca_pem {
        for cert in aithos_provider::tls::load_cert_chain(std::fs::read(pem)?.as_slice())? {
            roots.add(cert)?;
        }
    } else {
        let (_added, _ignored) = roots.add_parsable_certificates(webpki_roots_or_native()?);
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

/// Build the public server config from PEM bytes (the ACME-issued chain
/// and the locally generated key).
fn build_pod_tls_from_pems(
    chain_pem: &[u8],
    key_pem: &[u8],
) -> Result<Arc<rustls::ServerConfig>, Box<dyn std::error::Error>> {
    let chain = aithos_provider::tls::load_cert_chain(chain_pem)?;
    let key = aithos_provider::tls::load_private_key(key_pem)?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(chain, key)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

/// The pod's public-facing server config. A provided cert (a real LE cert)
/// wins; otherwise a self-signed cert for `hostname`, whose PEM we print
/// so the prober can `--cacert` it.
fn build_pod_tls(hostname: &str) -> Result<Arc<rustls::ServerConfig>, Box<dyn std::error::Error>> {
    let (chain, key) = match (std::env::var("POD_TLS_CERT"), std::env::var("POD_TLS_KEY")) {
        (Ok(cert_path), Ok(key_path)) => {
            let chain = aithos_provider::tls::load_cert_chain(&std::fs::read(cert_path)?)?;
            let key = aithos_provider::tls::load_private_key(&std::fs::read(key_path)?)?;
            (chain, key)
        }
        _ => {
            let cert = rcgen::generate_simple_self_signed(vec![hostname.to_owned()])?;
            let cert_pem = cert.cert.pem();
            eprintln!(
                "pod-stub: self-signed cert for {hostname}. Trust this CA to reach it:\n{cert_pem}"
            );
            let chain = aithos_provider::tls::load_cert_chain(cert_pem.as_bytes())?;
            let key =
                aithos_provider::tls::load_private_key(cert.key_pair.serialize_pem().as_bytes())?;
            (chain, key)
        }
    };
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(chain, key)?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

/// TLS client that trusts the relay tunnel door. `POD_RELAY_CA` pins a PEM
/// CA (a self-signed relay cert in tests); otherwise system roots.
fn build_relay_connector(
    ca_pem: Option<&str>,
) -> Result<tokio_rustls::TlsConnector, Box<dyn std::error::Error>> {
    let mut roots = rustls::RootCertStore::empty();
    if let Some(pem) = ca_pem {
        for cert in aithos_provider::tls::load_cert_chain(std::fs::read(pem)?.as_slice())? {
            roots.add(cert)?;
        }
    } else {
        let (_added, _ignored) = roots.add_parsable_certificates(webpki_roots_or_native()?);
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![TUNNEL_ALPN.to_vec()];
    Ok(tokio_rustls::TlsConnector::from(Arc::new(config)))
}

/// Native root certificates for the outbound relay TLS (the relay's cert
/// is a real one in prod). Kept behind a tiny helper so the dep stays
/// local to the pod-stub.
fn webpki_roots_or_native(
) -> Result<Vec<rustls::pki_types::CertificateDer<'static>>, Box<dyn std::error::Error>> {
    // The pod-stub reads the OS trust store via the standard bundle path;
    // in the FROM-scratch image the CA bundle is baked at the standard
    // location (same as the store-api image).
    let bundle = std::fs::read("/etc/ssl/certs/ca-certificates.crt")
        .or_else(|_| std::fs::read("/etc/ssl/cert.pem"))?;
    Ok(aithos_provider::tls::load_cert_chain(&bundle)?)
}
