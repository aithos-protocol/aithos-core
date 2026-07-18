//! Log discipline — annexe A.8, opposable, façon `credentials.rs`.
//!
//! One request = at most one log line, built HERE and nowhere else, from a
//! **closed register of fields**:
//!
//! - allowed — `at`, `tenant`, `did`, route class (closed enum), verb,
//!   HTTP status, error code (closed registry A.7), sizes, duration;
//! - forbidden — the full path, the query, any body, any envelope, any
//!   header value.
//!
//! The renderer takes no free string: the error code is `&'static str`
//! from [`crate::envelope::Refusal`], the class is an enum, tenant and did
//! only ever come from the pathmap's validated grammar (and are allowed
//! fields anyway). A handler that wants to log something else has nowhere
//! to put it — that is the point.

use crate::pathmap::{DataTarget, TargetKind};

/// The closed route-class enum of annexe A.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteClass {
    Read,
    List,
    Heads,
    Batch,
    PutArtifact,
    Publish,
    GammaAppend,
    GammaReplica,
    Sync,
    Acme,
}

impl RouteClass {
    pub fn as_str(self) -> &'static str {
        match self {
            RouteClass::Read => "read",
            RouteClass::List => "list",
            RouteClass::Heads => "heads",
            RouteClass::Batch => "batch",
            RouteClass::PutArtifact => "put_artifact",
            RouteClass::Publish => "publish",
            RouteClass::GammaAppend => "gamma_append",
            RouteClass::GammaReplica => "gamma_replica",
            RouteClass::Sync => "sync",
            RouteClass::Acme => "acme",
        }
    }

    /// Classify a parsed target. `None` for a request rejected before the
    /// grammar — no class exists for it in the closed register, so none is
    /// logged.
    pub fn of(target: &DataTarget, method: &str) -> Option<RouteClass> {
        Some(match (&target.kind, method) {
            (TargetKind::Object(crate::pathmap::ObjectPath::Manifest), "PUT") => {
                RouteClass::Publish
            }
            (TargetKind::Object(crate::pathmap::ObjectPath::GammaSegment(_)), "PUT") => {
                RouteClass::GammaReplica
            }
            (TargetKind::Object(_), "PUT") => RouteClass::PutArtifact,
            (TargetKind::Object(_), _) => RouteClass::Read,
            (TargetKind::Heads, _) => RouteClass::Heads,
            (TargetKind::Batch, _) => RouteClass::Batch,
            (TargetKind::Gamma, _) => RouteClass::GammaAppend,
            (TargetKind::Sync, _) => RouteClass::Sync,
            (TargetKind::List, _) => RouteClass::List,
        })
    }
}

/// The one request log line. Every field is from the allowed register.
pub struct RequestLine<'a> {
    pub at_ms: i64,
    pub tenant: Option<&'a str>,
    pub did: Option<&'a str>,
    pub class: Option<RouteClass>,
    pub verb: &'a str,
    pub status: u16,
    pub error: Option<&'static str>,
    pub req_bytes: usize,
    pub resp_bytes: usize,
    pub duration_ms: u128,
}

impl RequestLine<'_> {
    /// Render the line. Space-separated `k=v`, stable order, no free text.
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut line = String::with_capacity(160);
        let _ = write!(line, "at={}", crate::time::render_rfc3339z(self.at_ms));
        if let Some(tenant) = self.tenant {
            let _ = write!(line, " tenant={tenant}");
        }
        if let Some(did) = self.did {
            let _ = write!(line, " did={did}");
        }
        if let Some(class) = self.class {
            let _ = write!(line, " class={}", class.as_str());
        }
        let _ = write!(line, " verb={}", sanitized_verb(self.verb));
        let _ = write!(line, " status={}", self.status);
        if let Some(error) = self.error {
            let _ = write!(line, " error={error}");
        }
        let _ = write!(
            line,
            " req_bytes={} resp_bytes={} dur_ms={}",
            self.req_bytes, self.resp_bytes, self.duration_ms
        );
        line
    }

    /// Emit through `tracing` at the single request target.
    pub fn emit(&self) {
        tracing::info!(target: "aithos_store::request", "{}", self.render());
    }
}

/// Verbs are a closed HTTP set; anything exotic logs as `other` rather
/// than echoing attacker-controlled method bytes.
fn sanitized_verb(verb: &str) -> &'static str {
    match verb {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "DELETE" => "DELETE",
        _ => "other",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_line_carries_only_the_allowed_register() {
        let line = RequestLine {
            at_ms: 1_784_203_200_000,
            tenant: Some("acme"),
            did: Some("did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr"),
            class: Some(RouteClass::PutArtifact),
            verb: "PUT",
            status: 200,
            error: None,
            req_bytes: 8,
            resp_bytes: 2,
            duration_ms: 3,
        }
        .render();
        assert_eq!(
            line,
            "at=2026-07-16T12:00:00Z tenant=acme \
             did=did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr \
             class=put_artifact verb=PUT status=200 req_bytes=8 resp_bytes=2 dur_ms=3"
        );
    }

    #[test]
    fn exotic_verbs_never_echo() {
        let line = RequestLine {
            at_ms: 0,
            tenant: None,
            did: None,
            class: None,
            verb: "EVIL\r\nSet-Cookie: x",
            status: 400,
            error: Some("path_invalid"),
            req_bytes: 0,
            resp_bytes: 0,
            duration_ms: 0,
        }
        .render();
        assert!(!line.contains("EVIL"));
        assert!(line.contains("verb=other"));
        assert!(line.contains("error=path_invalid"));
    }
}
