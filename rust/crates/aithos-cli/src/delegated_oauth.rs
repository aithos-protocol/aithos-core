use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::error::Error;
use std::io::{Read as _, Write as _};
use std::path::PathBuf;
use std::time::Duration;
use url::Url;
use zeroize::{Zeroize as _, Zeroizing};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

pub struct AuthorizeOptions {
    pub gateway: String,
    pub signer_stdin: bool,
    pub token_output: PathBuf,
    pub approve: bool,
    pub context: Option<String>,
    pub parent_id: Option<String>,
    pub scope: Option<String>,
    pub redirect_uri: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingCeremony {
    transaction_id: String,
    client_id: String,
    resource: String,
    gateway_pub: String,
    gateway_kex_pub: String,
    session_pub: String,
    nonce: String,
    expires_at_epoch: i64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthorizeEnvelope {
    v: u8,
    ceremony: PendingCeremony,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CeremonyBindings {
    transaction_id: String,
    delegate_pub: String,
    client_id: String,
    redirect_uri: String,
    resource: String,
    code_challenge: String,
    scope: Option<String>,
    state_digest: Option<String>,
    gateway_pub: String,
    gateway_kex_pub: String,
    session_pub: String,
    nonce: String,
    expires_at_epoch: i64,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct EligibleParent {
    context: String,
    parent_id: String,
    subject: String,
    not_before: String,
    not_after: String,
    #[serde(rename = "perimeter")]
    _perimeter: Vec<String>,
    session_perimeter: Vec<String>,
    constraints: Value,
    chain: Vec<Value>,
    did: Value,
    revocations: Vec<Value>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PreparationEnvelope {
    v: u8,
    verified_at: String,
    bindings: CeremonyBindings,
    eligible_parents: Vec<EligibleParent>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantEnvelope {
    v: u8,
    grant: Value,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteEnvelope {
    redirect_to: String,
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(15)))
        .http_status_as_error(false)
        .max_redirects(0)
        .build()
        .into()
}

fn response_json(mut response: ureq::http::Response<ureq::Body>, operation: &str) -> Result<Value> {
    if !response.status().is_success() {
        return Err(format!(
            "{operation} was refused with HTTP status {}",
            response.status()
        )
        .into());
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(1024 * 1024)
        .read_to_vec()
        .map_err(|_| format!("{operation} returned an unreadable body"))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| format!("{operation} returned malformed JSON").into())
}

fn get_json(agent: &ureq::Agent, url: &Url, operation: &str) -> Result<Value> {
    let response = agent
        .get(url.as_str())
        .header("Accept", "application/json")
        .call()
        .map_err(|_| format!("{operation} transport failed"))?;
    response_json(response, operation)
}

fn post_json(agent: &ureq::Agent, url: &Url, body: &Value, operation: &str) -> Result<Value> {
    let bytes = serde_json::to_vec(body)?;
    let response = agent
        .post(url.as_str())
        .header("Accept", "application/json")
        .header("Content-Type", "application/json")
        .send(&bytes)
        .map_err(|_| format!("{operation} transport failed"))?;
    response_json(response, operation)
}

fn post_form(
    agent: &ureq::Agent,
    url: &Url,
    fields: &[(&str, &str)],
    operation: &str,
) -> Result<Value> {
    let body = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(fields.iter().copied())
        .finish();
    let response = agent
        .post(url.as_str())
        .header("Accept", "application/json")
        .header("Content-Type", "application/x-www-form-urlencoded")
        .send(body.as_bytes())
        .map_err(|_| format!("{operation} transport failed"))?;
    response_json(response, operation)
}

fn public_gateway_url(raw: &str) -> Result<Url> {
    let mut url = Url::parse(raw).map_err(|_| "gateway URL is malformed")?;
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !matches!(url.path(), "" | "/" | "/mcp")
    {
        return Err("gateway URL must be an origin or its /mcp resource without credentials, query or fragment".into());
    }
    let loopback = matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"));
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err("gateway URL must use HTTPS (HTTP is allowed only on loopback)".into());
    }
    url.set_path("/");
    Ok(url)
}

fn loopback_redirect(raw: &str) -> Result<Url> {
    let url = Url::parse(raw).map_err(|_| "redirect URI is malformed")?;
    if url.scheme() != "http"
        || !matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err("redirect URI must be a credential-free HTTP loopback URL".into());
    }
    Ok(url)
}

fn same_origin(left: &Url, right: &Url) -> bool {
    left.scheme() == right.scheme()
        && left.host_str() == right.host_str()
        && left.port_or_known_default() == right.port_or_known_default()
}

fn endpoint(metadata: &Value, name: &str, origin: &Url) -> Result<Url> {
    let value = metadata
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| format!("authorization metadata omits {name}"))?;
    let url =
        Url::parse(value).map_err(|_| format!("authorization metadata has malformed {name}"))?;
    if !same_origin(&url, origin) || url.scheme() != origin.scheme() {
        return Err(format!("authorization metadata moves {name} off the gateway origin").into());
    }
    Ok(url)
}

fn random<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0u8; N];
    std::fs::File::open("/dev/urandom")?.read_exact(&mut bytes)?;
    Ok(bytes)
}

fn b64url(bytes: &[u8]) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn read_signer_seed() -> Result<Zeroizing<[u8; 32]>> {
    let mut input = Zeroizing::new(String::new());
    std::io::stdin()
        .lock()
        .take(1024)
        .read_to_string(&mut input)?;
    let mut decoded = Zeroizing::new(
        hex::decode(input.trim())
            .map_err(|_| "stdin signer must be exactly 32 hexadecimal bytes")?,
    );
    input.zeroize();
    let seed: [u8; 32] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| "stdin signer must be exactly 32 hexadecimal bytes")?;
    decoded.zeroize();
    Ok(Zeroizing::new(seed))
}

fn selected_parent<'a>(
    parents: &'a [EligibleParent],
    context: Option<&str>,
    parent_id: Option<&str>,
) -> Result<&'a EligibleParent> {
    let matching = parents
        .iter()
        .filter(|parent| context.is_none_or(|value| parent.context == value))
        .filter(|parent| parent_id.is_none_or(|value| parent.parent_id == value))
        .collect::<Vec<_>>();
    match matching.as_slice() {
        [parent] => Ok(parent),
        [] => Err("no eligible parent matches --context/--parent-id".into()),
        _ => Err("multiple parents are eligible; pass --context and/or --parent-id".into()),
    }
}

fn timestamp_window(parent: &EligibleParent, verified_at: &str) -> Result<(String, String)> {
    let verified = aithos_core::gamma::ts_epoch(verified_at)?;
    let parent_before = aithos_core::gamma::ts_epoch(&parent.not_before)?;
    let parent_after = aithos_core::gamma::ts_epoch(&parent.not_after)?;
    let before = verified.max(parent_before);
    let after = parent_after.min(verified + 8 * 60 * 60);
    if after <= before {
        return Err("eligible parent has no usable delegated-session window".into());
    }
    Ok((
        crate::cmd::common::ts(before as u64),
        crate::cmd::common::ts(after as u64),
    ))
}

fn write_tokens(path: &PathBuf, document: &Value) -> Result<()> {
    let bytes = Zeroizing::new(serde_json::to_vec_pretty(document)?);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "token output must be a new writable file")?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    Ok(())
}

pub fn authorize_delegated(options: AuthorizeOptions) -> Result<()> {
    if !options.signer_stdin || !options.approve {
        return Err("production authorization requires --signer-stdin and --approve".into());
    }
    if options.token_output.exists() {
        return Err("token output already exists; refusing to overwrite it".into());
    }
    let gateway = public_gateway_url(&options.gateway)?;
    let redirect = loopback_redirect(&options.redirect_uri)?;
    let mut seed = read_signer_seed()?;
    let signer = aithos_wasm::DelegateSigner::new(seed.as_mut())
        .map_err(|_| "stdin signer could not be initialized")?;
    seed.zeroize();
    let delegate_pub = signer.public_key();

    let http = agent();
    let protected_url = gateway.join(".well-known/oauth-protected-resource")?;
    let protected = get_json(&http, &protected_url, "protected-resource discovery")?;
    let resource = protected
        .get("resource")
        .and_then(Value::as_str)
        .ok_or("protected-resource metadata omits resource")?
        .to_owned();
    let resource_url = Url::parse(&resource).map_err(|_| "protected resource is malformed")?;
    if !same_origin(&gateway, &resource_url) {
        return Err("protected resource is off the gateway origin".into());
    }
    let issuer = protected
        .get("authorization_servers")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(Value::as_str)
        .ok_or("protected-resource metadata omits its authorization server")?;
    let issuer = Url::parse(issuer).map_err(|_| "authorization server URL is malformed")?;
    if !same_origin(&gateway, &issuer) {
        return Err("authorization server is off the gateway origin".into());
    }
    let metadata_url = gateway.join(".well-known/oauth-authorization-server")?;
    let metadata = get_json(&http, &metadata_url, "authorization-server discovery")?;
    let register_url = endpoint(&metadata, "registration_endpoint", &gateway)?;
    let authorize_endpoint = endpoint(&metadata, "authorization_endpoint", &gateway)?;
    let token_url = endpoint(&metadata, "token_endpoint", &gateway)?;

    let registration = post_json(
        &http,
        &register_url,
        &json!({
            "client_name": "Aithos delegated CLI",
            "redirect_uris": [redirect.as_str()],
            "token_endpoint_auth_method": "none",
            "grant_types": ["authorization_code", "refresh_token"],
            "response_types": ["code"],
        }),
        "dynamic client registration",
    )?;
    let client_id = registration
        .get("client_id")
        .and_then(Value::as_str)
        .ok_or("registration response omits client_id")?
        .to_owned();

    let verifier = Zeroizing::new(b64url(&random::<32>()?));
    let challenge = b64url(&Sha256::digest(verifier.as_bytes()));
    let state = b64url(&random::<16>()?);
    let mut authorize_url = authorize_endpoint;
    {
        let mut query = authorize_url.query_pairs_mut();
        query
            .append_pair("client_id", &client_id)
            .append_pair("redirect_uri", redirect.as_str())
            .append_pair("response_type", "code")
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256")
            .append_pair("resource", &resource)
            .append_pair("state", &state);
        if let Some(scope) = &options.scope {
            query.append_pair("scope", scope);
        }
    }
    let authorize: AuthorizeEnvelope = serde_json::from_value(get_json(
        &http,
        &authorize_url,
        "delegated authorization start",
    )?)
    .map_err(|_| "delegated authorization start returned an unexpected document")?;
    if authorize.v != 1
        || authorize.ceremony.client_id != client_id
        || authorize.ceremony.resource != resource
    {
        return Err("delegated authorization start binding mismatch".into());
    }

    let prepare_url = gateway.join("ceremony/prepare")?;
    let preparation: PreparationEnvelope = serde_json::from_value(post_json(
        &http,
        &prepare_url,
        &json!({
            "transaction_id": authorize.ceremony.transaction_id,
            "delegate_pub": delegate_pub,
        }),
        "delegated ceremony preparation",
    )?)
    .map_err(|_| "delegated ceremony preparation returned an unexpected document")?;
    if preparation.v != 1
        || preparation.bindings.transaction_id != authorize.ceremony.transaction_id
        || preparation.bindings.delegate_pub != delegate_pub
        || preparation.bindings.client_id != client_id
        || preparation.bindings.resource != resource
        || preparation.bindings.gateway_pub != authorize.ceremony.gateway_pub
        || preparation.bindings.gateway_kex_pub != authorize.ceremony.gateway_kex_pub
        || preparation.bindings.session_pub != authorize.ceremony.session_pub
        || preparation.bindings.nonce != authorize.ceremony.nonce
        || preparation.bindings.expires_at_epoch != authorize.ceremony.expires_at_epoch
    {
        return Err("delegated ceremony preparation binding mismatch".into());
    }
    let parent = selected_parent(
        &preparation.eligible_parents,
        options.context.as_deref(),
        options.parent_id.as_deref(),
    )?;
    let chain_json = serde_json::to_string(&parent.chain)?;
    let did_json = serde_json::to_string(&parent.did)?;
    let revocations_json = serde_json::to_string(&parent.revocations)?;
    aithos_wasm::verify_mandate_chain(
        &chain_json,
        &did_json,
        &preparation.verified_at,
        Some(revocations_json),
    )
    .map_err(|_| "eligible parent chain failed local Core verification")?;
    let (not_before, not_after) = timestamp_window(parent, &preparation.verified_at)?;
    let leaf_id = format!(
        "mandate_{}",
        ulid::Ulid::from(u128::from_be_bytes(random::<16>()?))
    );
    let request = json!({
        "id": leaf_id,
        "subject": parent.subject,
        "grantee_id": format!("urn:aithos:agent:mcp-session-{}", hex::encode(random::<8>()?)),
        "grantee_label": "MCP delegated session",
        "gateway_pub": preparation.bindings.gateway_pub,
        "gateway_kex_pub": preparation.bindings.gateway_kex_pub,
        "session_pub": preparation.bindings.session_pub,
        "perimeter": parent.session_perimeter,
        "constraints": parent.constraints,
        "not_before": not_before,
        "not_after": not_after,
        "issued_at": preparation.verified_at,
        "nonce": hex::encode(random::<16>()?),
    });
    let parent_json = serde_json::to_string(parent.chain.last().ok_or("eligible chain is empty")?)?;
    let leaf_json = signer
        .build_session_submandate(&parent_json, &serde_json::to_string(&request)?)
        .map_err(|_| "session leaf construction failed local Core attenuation")?;
    let leaf: Value = serde_json::from_str(&leaf_json)?;
    let mut complete_chain = parent.chain.clone();
    complete_chain.push(leaf.clone());
    aithos_wasm::verify_mandate_chain(
        &serde_json::to_string(&complete_chain)?,
        &did_json,
        &preparation.verified_at,
        Some(serde_json::to_string(&parent.revocations)?),
    )
    .map_err(|_| "session leaf failed local Core verification")?;

    let grant_url = gateway.join("ceremony/prepare-grant")?;
    let grant: GrantEnvelope = serde_json::from_value(post_json(
        &http,
        &grant_url,
        &json!({
            "transaction_id": preparation.bindings.transaction_id,
            "delegate_pub": delegate_pub,
            "context": parent.context,
            "parent_id": parent.parent_id,
            "leaf": leaf,
        }),
        "delegated grant preparation",
    )?)
    .map_err(|_| "delegated grant preparation returned an unexpected document")?;
    if grant.v != 1 {
        return Err("delegated grant preparation version mismatch".into());
    }
    let signed_grant_json = signer
        .sign_delegated_grant(&serde_json::to_string(&grant.grant)?)
        .map_err(|_| "delegated grant failed local Core signing checks")?;
    let signed_grant: Value = serde_json::from_str(&signed_grant_json)?;
    let challenge_json = aithos_wasm::build_ceremony_challenge(
        &serde_json::to_string(&preparation.bindings)?,
        &parent.context,
        &parent.parent_id,
        &leaf_json,
        &signed_grant_json,
    )
    .map_err(|_| "WYSIWYS challenge construction failed")?;
    let challenge_envelope: Value = serde_json::from_str(&challenge_json)?;
    let digest = challenge_envelope
        .get("digest")
        .and_then(Value::as_str)
        .ok_or("WYSIWYS challenge omits its digest")?;
    let challenge = challenge_envelope
        .get("challenge")
        .cloned()
        .ok_or("WYSIWYS challenge omits its closed payload")?;

    println!("gateway: {}", gateway.as_str());
    println!("oauth_client: {client_id}");
    println!("resource: {resource}");
    println!("delegate_pub: {delegate_pub}");
    println!("context: {}", parent.context);
    println!(
        "mandate_chain: {}",
        complete_chain
            .iter()
            .filter_map(|mandate| mandate.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(" -> ")
    );
    println!("parent_id: {}", parent.parent_id);
    println!("leaf_id: {leaf_id}");
    println!("perimeter: {}", serde_json::to_string(&leaf["perimeter"])?);
    println!(
        "constraints: {}",
        serde_json::to_string(&leaf["constraints"])?
    );
    println!("session_pub: {}", preparation.bindings.session_pub);
    println!("not_before: {not_before}");
    println!("not_after: {not_after}");
    println!("wysiwys_digest: {digest}");
    println!("local_verification: OK");

    let proof_json = signer
        .sign_ceremony_challenge(&serde_json::to_string(&challenge)?)
        .map_err(|_| "WYSIWYS ceremony proof could not be signed")?;
    let proof: Value = serde_json::from_str(&proof_json)?;
    let complete_url = gateway.join("ceremony/complete")?;
    let completed: CompleteEnvelope = serde_json::from_value(post_json(
        &http,
        &complete_url,
        &json!({
            "transaction_id": preparation.bindings.transaction_id,
            "context": parent.context,
            "parent_id": parent.parent_id,
            "leaf": leaf,
            "grant": signed_grant,
            "proof": proof,
        }),
        "delegated ceremony completion",
    )?)
    .map_err(|_| "delegated ceremony completion returned an unexpected document")?;
    let callback = Url::parse(&completed.redirect_to)
        .map_err(|_| "delegated ceremony returned a malformed callback")?;
    if callback.scheme() != redirect.scheme()
        || callback.host_str() != redirect.host_str()
        || callback.port_or_known_default() != redirect.port_or_known_default()
        || callback.path() != redirect.path()
    {
        return Err("delegated ceremony returned an unexpected callback origin or path".into());
    }
    let callback_pairs = callback
        .query_pairs()
        .collect::<std::collections::BTreeMap<_, _>>();
    let callback_state = callback_pairs
        .get("state")
        .ok_or("OAuth callback omits state")?;
    if callback_state.as_ref() != state {
        return Err("OAuth callback state mismatch".into());
    }
    let code = Zeroizing::new(
        callback_pairs
            .get("code")
            .ok_or("OAuth callback omits authorization code")?
            .to_string(),
    );
    let token = post_form(
        &http,
        &token_url,
        &[
            ("grant_type", "authorization_code"),
            ("code", code.as_str()),
            ("code_verifier", verifier.as_str()),
            ("resource", &resource),
            ("redirect_uri", redirect.as_str()),
        ],
        "authorization-code exchange",
    )?;
    let access_token = Zeroizing::new(
        token
            .get("access_token")
            .and_then(Value::as_str)
            .ok_or("token response omits access_token")?
            .to_owned(),
    );
    let refresh_token = Zeroizing::new(
        token
            .get("refresh_token")
            .and_then(Value::as_str)
            .ok_or("token response omits refresh_token")?
            .to_owned(),
    );
    let expires_in = token
        .get("expires_in")
        .and_then(Value::as_i64)
        .ok_or("token response omits expires_in")?;
    if token.get("token_type").and_then(Value::as_str) != Some("Bearer") {
        return Err("token response is not a Bearer grant".into());
    }
    write_tokens(
        &options.token_output,
        &json!({
            "v": 1,
            "resource": resource,
            "client_id": client_id,
            "token_type": "Bearer",
            "expires_in": expires_in,
            "access_token": access_token.as_str(),
            "refresh_token": refresh_token.as_str(),
        }),
    )?;
    println!("oauth_callback: verified");
    println!("token_output: {}", options.token_output.display());
    println!("authorization: OK");
    Ok(())
}
