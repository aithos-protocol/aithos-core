#[path = "fixtures/vectors.rs"]
mod fixtures_vectors;

use assert_cmd::Command;
use base64::Engine as _;
use serde_json::{json, Value};
use sha2::{Digest as _, Sha256};
use std::io::{Read as _, Write as _};
use tempfile::TempDir;
use url::Url;

const DELEGATE_SEED: &str = "6262626262626262626262626262626262626262626262626262626262626262";
const ACCESS_TOKEN: &str = "access.secret-never-print";
const REFRESH_TOKEN: &str = "refresh.secret-never-print";

fn ac() -> Command {
    Command::cargo_bin("aithos").expect("binary builds")
}

fn read_request(mut stream: &std::net::TcpStream) -> (String, Vec<u8>) {
    let mut bytes = Vec::new();
    let mut buffer = [0u8; 4096];
    loop {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0, "request ended before its headers");
        bytes.extend_from_slice(&buffer[..count]);
        if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
            break;
        }
    }
    let split = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    let headers = String::from_utf8(bytes[..split].to_vec()).unwrap();
    let content_length = headers
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("content-length: ")
                .map(str::to_owned)
        })
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    while bytes.len() - split < content_length {
        let count = stream.read(&mut buffer).unwrap();
        assert!(count > 0, "request ended before its body");
        bytes.extend_from_slice(&buffer[..count]);
    }
    (headers, bytes[split..split + content_length].to_vec())
}

fn answer_json(mut stream: std::net::TcpStream, value: &Value) {
    let body = serde_json::to_vec(value).unwrap();
    write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    )
    .unwrap();
    stream.write_all(&body).unwrap();
}

fn request_target(headers: &str, method: &str) -> String {
    let line = headers.lines().next().unwrap();
    let prefix = format!("{method} ");
    assert!(line.starts_with(&prefix), "unexpected request: {line}");
    line[prefix.len()..].split(' ').next().unwrap().to_owned()
}

#[test]
fn delegated_oauth_flow_uses_stdin_core_signing_and_a_private_token_file() {
    let vector: Value = serde_json::from_str(&fixtures_vectors::vector_str(
        "cb15-external-delegated-grant.json",
    ))
    .unwrap();
    let positive = vector["positive"].clone();
    let parent = positive["minting_chain"][0].clone();
    let did = positive["did"].clone();
    let delegate_pub = parent["grantee"]["pubkey"].as_str().unwrap().to_owned();
    let gateway_pub = positive["child"]["grantee"]["pubkey"]
        .as_str()
        .unwrap()
        .to_owned();
    let gateway_kex_pub = positive["child"]["grantee"]["kex_pubkey"]
        .as_str()
        .unwrap()
        .to_owned();
    let session_pub = positive["child"]["constraints"]["session_bind"]
        .as_str()
        .unwrap()
        .to_owned();

    let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server_origin = origin.clone();
    let server = std::thread::spawn(move || {
        let resource = format!("{server_origin}/mcp");
        let redirect = "http://127.0.0.1/aithos/callback";
        let transaction = "ceremony_cli_test";
        let client_id = "client_cli_test";
        let nonce = "abababababababababababababababab";

        let (stream, _) = listener.accept().unwrap();
        let (headers, _) = read_request(&stream);
        assert_eq!(
            request_target(&headers, "GET"),
            "/.well-known/oauth-protected-resource"
        );
        answer_json(
            stream,
            &json!({
                "resource": resource,
                "authorization_servers": [server_origin],
            }),
        );

        let (stream, _) = listener.accept().unwrap();
        let (headers, _) = read_request(&stream);
        assert_eq!(
            request_target(&headers, "GET"),
            "/.well-known/oauth-authorization-server"
        );
        answer_json(
            stream,
            &json!({
                "issuer": server_origin,
                "registration_endpoint": format!("{server_origin}/register"),
                "authorization_endpoint": format!("{server_origin}/authorize"),
                "token_endpoint": format!("{server_origin}/token"),
            }),
        );

        let (stream, _) = listener.accept().unwrap();
        let (headers, body) = read_request(&stream);
        assert_eq!(request_target(&headers, "POST"), "/register");
        let registration: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(registration["token_endpoint_auth_method"], "none");
        assert_eq!(registration["redirect_uris"][0], redirect);
        answer_json(stream, &json!({ "client_id": client_id }));

        let (stream, _) = listener.accept().unwrap();
        let (headers, _) = read_request(&stream);
        assert!(headers
            .to_ascii_lowercase()
            .contains("accept: application/json"));
        let target = request_target(&headers, "GET");
        let authorize = Url::parse(&format!("http://gateway{target}")).unwrap();
        let query = authorize
            .query_pairs()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(query["client_id"], client_id);
        assert_eq!(query["redirect_uri"], redirect);
        assert_eq!(query["resource"], resource);
        let challenge = query["code_challenge"].to_string();
        let state = query["state"].to_string();
        answer_json(
            stream,
            &json!({
                "v": 1,
                "ceremony": {
                    "transaction_id": transaction,
                    "client_id": client_id,
                    "resource": resource,
                    "gateway_pub": gateway_pub,
                    "gateway_kex_pub": gateway_kex_pub,
                    "session_pub": session_pub,
                    "nonce": nonce,
                    "expires_at_epoch": 1784720400_i64,
                }
            }),
        );

        let (stream, _) = listener.accept().unwrap();
        let (headers, body) = read_request(&stream);
        assert_eq!(request_target(&headers, "POST"), "/ceremony/prepare");
        let preparation: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(preparation["transaction_id"], transaction);
        assert_eq!(preparation["delegate_pub"], delegate_pub);
        answer_json(
            stream,
            &json!({
                "v": 1,
                "verified_at": "2026-07-22T11:30:00Z",
                "bindings": {
                    "transaction_id": transaction,
                    "delegate_pub": delegate_pub,
                    "client_id": client_id,
                    "redirect_uri": redirect,
                    "resource": resource,
                    "code_challenge": challenge,
                    "scope": null,
                    "state_digest": "sha256:state",
                    "gateway_pub": gateway_pub,
                    "gateway_kex_pub": gateway_kex_pub,
                    "session_pub": session_pub,
                    "nonce": nonce,
                    "expires_at_epoch": 1784720400_i64,
                },
                "eligible_parents": [{
                    "context": "finance",
                    "parent_id": parent["id"],
                    "subject": parent["subject"],
                    "not_before": parent["not_before"],
                    "not_after": parent["not_after"],
                    "perimeter": parent["perimeter"],
                    "session_perimeter": ["act.x.github.*"],
                    "constraints": parent["constraints"],
                    "chain": [parent],
                    "did": did,
                    "revocations": [],
                }],
            }),
        );

        let (stream, _) = listener.accept().unwrap();
        let (headers, body) = read_request(&stream);
        assert_eq!(request_target(&headers, "POST"), "/ceremony/prepare-grant");
        let grant_request: Value = serde_json::from_slice(&body).unwrap();
        let leaf = grant_request["leaf"].clone();
        assert_eq!(leaf["signature"]["key"], delegate_pub);
        assert_ne!(leaf["signature"]["value"], "");
        assert_eq!(leaf["constraints"]["session_bind"], session_pub);
        let leaf_id = leaf["id"].as_str().unwrap();
        let parent_id = grant_request["parent_id"].as_str().unwrap();
        answer_json(
            stream,
            &json!({
                "v": 1,
                "grant": {
                    "v": 1,
                    "id": "gamma_01J00000000000000000000CLI",
                    "prev": "",
                    "at": "2026-07-22T11:30:00Z",
                    "kind": "grant",
                    "target": leaf_id,
                    "authorized_by": parent_id,
                    "authorized_via": [parent_id],
                    "payload": {},
                    "signature": {
                        "alg": "ed25519",
                        "key": delegate_pub,
                        "value": "",
                    },
                }
            }),
        );

        let (stream, _) = listener.accept().unwrap();
        let (headers, body) = read_request(&stream);
        assert_eq!(request_target(&headers, "POST"), "/ceremony/complete");
        let completed: Value = serde_json::from_slice(&body).unwrap();
        assert_ne!(completed["grant"]["signature"]["value"], "");
        assert_eq!(completed["proof"]["delegate_pub"], delegate_pub);
        assert_ne!(completed["proof"]["sig"], "");
        answer_json(
            stream,
            &json!({
                "redirect_to": format!("{redirect}?code=one-shot-code&state={state}")
            }),
        );

        let (stream, _) = listener.accept().unwrap();
        let (headers, body) = read_request(&stream);
        assert_eq!(request_target(&headers, "POST"), "/token");
        let form = url::form_urlencoded::parse(&body).collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code"], "one-shot-code");
        assert_eq!(form["resource"], resource);
        let computed = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(Sha256::digest(form["code_verifier"].as_bytes()));
        assert_eq!(computed, challenge);
        answer_json(
            stream,
            &json!({
                "access_token": ACCESS_TOKEN,
                "token_type": "Bearer",
                "expires_in": 900,
                "refresh_token": REFRESH_TOKEN,
            }),
        );
    });

    let output_dir = TempDir::new().unwrap();
    let token_path = output_dir.path().join("oauth.json");
    let assertion = ac()
        .args([
            "oauth",
            "authorize-delegated",
            "--gateway",
            &origin,
            "--signer-stdin",
            "--token-output",
            token_path.to_str().unwrap(),
            "--approve",
        ])
        .write_stdin(format!("{DELEGATE_SEED}\n"))
        .assert()
        .success();
    let output = assertion.get_output();
    let visible = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(visible.contains("local_verification: OK"));
    assert!(visible.contains("authorization: OK"));
    assert!(visible.contains("oauth_client: client_cli_test"));
    assert!(visible.contains("perimeter: [\"act.x.github.*\"]"));
    assert!(visible.contains("constraints:"));
    assert!(visible.contains("wysiwys_digest: sha256:"));
    for secret in [DELEGATE_SEED, ACCESS_TOKEN, REFRESH_TOKEN, "one-shot-code"] {
        assert!(!visible.contains(secret));
    }

    let tokens: Value = serde_json::from_slice(&std::fs::read(&token_path).unwrap()).unwrap();
    assert_eq!(tokens["access_token"], ACCESS_TOKEN);
    assert_eq!(tokens["refresh_token"], REFRESH_TOKEN);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        assert_eq!(
            std::fs::metadata(&token_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    server.join().unwrap();
}
