//! Gamma log over the bundle (spec §07): monthly segments, authorized
//! appends, owner-first querying. The chain is the truth; everything here
//! reads files and delegates verdicts to aithos-core.

use crate::bundle::{Bundle, ZoneIndex, KV};
use crate::entropy::EntropySource;
use crate::Store;
use aithos_core::derive::node_key;
use aithos_core::error::{Error, Result};
use aithos_core::gamma::{
    self, check_action_append, check_grant_append, delegated_entry, open_body, owner_entry,
    seal_body, verify_delegated_entry, verify_links, Body, Entry, EntrySpec, Kind,
};
use aithos_core::ids::Sid;
use aithos_core::keys::{grantee_kex_secret, OwnerKeys};
use aithos_core::mandate::{covers_act, covers_gamma_query, ActOp, GammaQuery, Mandate};
use aithos_core::path::{Leaf, NodePath, Zone};
use ed25519_dalek::SigningKey;
use std::collections::BTreeMap;

fn io_err(e: std::io::Error) -> Error {
    Error::SealRejected(format!("store i/o: {e}"))
}

fn segment_of(at: &str) -> Result<String> {
    gamma::ts_epoch(at)?; // strict format gate
    Ok(format!("gamma/{}.jsonl", &at[..7]))
}

/// An owner-side query filter (§07.8). Every present dimension narrows.
#[derive(Debug, Clone, Default)]
pub struct LogFilter {
    pub kind: Option<String>,
    pub action: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    /// Display folder path (resolved against the zone index).
    pub zone_dir: Option<(Zone, String)>,
    pub tag: Option<String>,
    pub mandate: Option<String>,
}

/// One query hit: the entry, plus its opened body when a key reached it.
#[derive(Debug, Clone)]
pub struct LogHit {
    pub entry: Entry,
    pub body: Option<Body>,
}

/// Parameters of one logged connector action (§07.4, §04.11, §07.9.3).
#[derive(Debug, Clone, Default)]
pub struct ActionSpec<'a> {
    pub connector: &'a str,
    pub action: &'a str,
    /// SHA-256 of the action arguments — the log never carries clear args.
    /// Recomputed from `sealed_args` when those are provided.
    pub args_hash: &'a str,
    /// Injected timestamp of the entry.
    pub now: &'a str,
    /// Budget citation (§04.11): budget_ref / model / tokens / receipt,
    /// merged into the clear payload.
    pub budget: Option<serde_json::Value>,
    /// Full argument object, sealed under the connector's audit key
    /// (§07.9.3) for a-posteriori predicate audit.
    pub sealed_args: Option<serde_json::Value>,
}

/// Parameters of one metered inference (§07.9.1).
#[derive(Debug, Clone)]
pub struct InferenceSpec<'a> {
    pub provider: &'a str,
    pub model: &'a str,
    pub tokens_in: u64,
    pub tokens_out: u64,
    pub budget_ref: Option<&'a str>,
    pub now: &'a str,
}

impl<S: Store> Bundle<S> {
    // ------------------------------------------------------------ storage

    fn gamma_segments(&self) -> Result<Vec<String>> {
        let mut segs: Vec<String> = self
            .store
            .list("gamma")
            .map_err(io_err)?
            .into_iter()
            .filter(|p| p.ends_with(".jsonl"))
            .collect();
        segs.sort();
        Ok(segs)
    }

    /// The whole log, segment by segment, in chain order.
    pub fn gamma_entries(&self) -> Result<Vec<Entry>> {
        let mut out = Vec::new();
        for seg in self.gamma_segments()? {
            let bytes = self.get(&seg)?;
            for line in bytes.split(|b| *b == b'\n').filter(|l| !l.is_empty()) {
                let e: Entry = serde_json::from_slice(line)
                    .map_err(|e| Error::InvalidGammaChain(format!("{seg}: {e}")))?;
                out.push(e);
            }
        }
        Ok(out)
    }

    /// Current tip (`sha256:<hex>`, empty for an empty log).
    pub fn gamma_head(&self) -> Result<String> {
        gamma::head(&self.gamma_entries()?)
    }

    /// Raw append: the entry must chain on the current head and be
    /// well-formed. Authority checks happen in the callers below.
    pub fn gamma_append(&mut self, entry: &Entry) -> Result<()> {
        entry.check_form()?;
        let entries = self.gamma_entries()?;
        let head = gamma::head(&entries)?;
        if entry.prev != head {
            return Err(Error::InvalidGammaChain(format!(
                "{}: prev is not the current head",
                entry.id
            )));
        }
        if let Some(last) = entries.last() {
            if gamma::ts_epoch(&entry.at)? < gamma::ts_epoch(&last.at)? {
                return Err(Error::InvalidGammaChain(format!(
                    "{}: at goes backward",
                    entry.id
                )));
            }
        }
        let seg = segment_of(&entry.at)?;
        let mut bytes = self.store.get(&seg).map_err(io_err)?.unwrap_or_default();
        bytes.extend_from_slice(aithos_core::jcs::canonicalize(entry)?.as_bytes());
        bytes.push(b'\n');
        self.store.put(&seg, &bytes).map_err(io_err)
    }

    fn next_gamma_id(&self, ent: &mut dyn EntropySource) -> String {
        format!(
            "gamma_{}",
            Sid(ulid::Ulid::from(u128::from_be_bytes(ent.e16())))
        )
    }

    // ------------------------------------------------------ owner appends

    /// Log an owner content mutation (§07.2, §07.3): sealed body on keyed
    /// zones, clear target+payload on public.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn log_owner_mutation(
        &mut self,
        owner: &OwnerKeys,
        kind: Kind,
        node: &NodePath,
        payload: serde_json::Value,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let prev = self.gamma_head()?;
        let spec = match node.zone {
            Zone::Public => EntrySpec {
                id: self.next_gamma_id(ent),
                prev,
                at: now.to_owned(),
                kind,
                target: Some(node.to_string()),
                payload: Some(payload),
                body_enc: None,
            },
            _ => {
                let key = node_key(&self.zone_dk(node.zone, owner)?, node);
                let body = seal_body(&key, &self.did, &node.to_string(), KV, &payload, &ent.e24())?;
                EntrySpec {
                    id: self.next_gamma_id(ent),
                    prev,
                    at: now.to_owned(),
                    kind,
                    target: None,
                    payload: None,
                    body_enc: Some(body),
                }
            }
        };
        let entry = owner_entry(spec, &owner.content_sign)?;
        self.gamma_append(&entry)
    }

    /// Log an owner modification of an existing section by display path
    /// (§07.3): sealed on keyed zones, clear on public.
    pub fn log_section_modify(
        &mut self,
        owner: &OwnerKeys,
        zone: Zone,
        display_path: &str,
        payload: serde_json::Value,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let (row, folders) = self.resolve_clear(zone, display_path)?;
        let node = NodePath::section(zone, folders, Sid::parse(&row.sid)?);
        self.log_owner_mutation(owner, Kind::SectionModify, &node, payload, now, ent)
    }

    /// Owner liveness beacon (§07.5).
    pub fn log_heartbeat(
        &mut self,
        owner: &OwnerKeys,
        seq: u64,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let entry = owner_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: self.gamma_head()?,
                at: now.to_owned(),
                kind: Kind::Heartbeat,
                target: None,
                payload: Some(serde_json::json!({ "seq": seq })),
                body_enc: None,
            },
            &owner.content_sign,
        )?;
        self.gamma_append(&entry)
    }

    /// Owner-issued grant, logged (§07.4: issuance is never silent).
    /// Called by the driver (CLI/steps) right after `grant()`.
    pub fn log_owner_grant(
        &mut self,
        owner: &OwnerKeys,
        child_mandate_id: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let entry = owner_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: self.gamma_head()?,
                at: now.to_owned(),
                kind: Kind::Grant,
                target: Some(child_mandate_id.to_owned()),
                payload: Some(serde_json::json!({})),
                body_enc: None,
            },
            &owner.content_sign,
        )?;
        self.gamma_append(&entry)
    }

    // -------------------------------------------------- delegated appends

    /// A connector action under a mandate chain (§07.4): verified against
    /// the chain at its own `at`, budget-checked, then appended. The entry
    /// IS the authorization evidence — no entry, no action (I5).
    pub fn log_action(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        spec: &ActionSpec<'_>,
        ent: &mut dyn EntropySource,
    ) -> Result<Entry> {
        let doc = self.did_doc()?;
        let entries = self.gamma_entries()?;
        let via: Vec<String> = chain.iter().map(|m| m.id.clone()).collect();
        // Sealed args (§07.9.3): args_hash pins the sealed body.
        let (args_hash, body_enc) = match &spec.sealed_args {
            None => (spec.args_hash.to_owned(), None),
            Some(args) => {
                let canon = aithos_core::jcs::canonical_bytes(args)?;
                let hash = format!("sha256:{}", aithos_core::gamma::sha256_hex(&canon));
                let key = self.audit_key_any(chain, agent_sk, spec.connector)?;
                let target = format!("x.{}", spec.connector);
                let body =
                    aithos_core::gamma::seal_body(&key, &self.did, &target, KV, args, &ent.e24())?;
                (hash, Some(body))
            }
        };
        let mut payload = serde_json::json!({
            "action": spec.action,
            "args_hash": args_hash,
        });
        if let Some(budget) = &spec.budget {
            if let (Some(p), Some(b)) = (payload.as_object_mut(), budget.as_object()) {
                for (k, v) in b {
                    p.insert(k.clone(), v.clone());
                }
            }
        }
        let entry = delegated_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: gamma::head(&entries)?,
                at: spec.now.to_owned(),
                kind: Kind::Action,
                target: Some(format!("x.{}", spec.connector)),
                payload: Some(payload),
                body_enc,
            },
            via,
            agent_sk,
        )?;
        verify_delegated_entry(&entry, chain, &doc)?;
        check_action_append(&entries, &entry, chain, &doc)?;
        self.gamma_append(&entry)?;
        Ok(entry)
    }

    /// The connector audit key (§07.9.3): owner-derivable from the vault
    /// root; an agent uses a granted header line on `/x` or `/x/<connector>`.
    fn audit_key_any(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        connector: &str,
    ) -> Result<[u8; 32]> {
        let label = format!("aithos-core/v1/x/{connector}");
        // Agent path: a line on the vault root sealed to the leaf grantee.
        if let Some(leaf) = chain.last() {
            let kex = grantee_kex_secret(agent_sk);
            if let Some(bytes) = self.store.get("e/x/header.json").ok().flatten() {
                if let Ok(header) = serde_json::from_slice::<aithos_core::header::Header>(&bytes) {
                    if let Ok(dk) = header.open(&self.did, KV, &leaf.grantee.pubkey, &kex) {
                        return Ok(aithos_core::derive::derive_key(&label, &dk));
                    }
                }
            }
        }
        Err(Error::SealRejected(format!(
            "no audit key path for connector {connector}"
        )))
    }

    /// Owner-side audit key for a connector (§07.9.3).
    pub fn audit_key_owner(&self, owner: &OwnerKeys, connector: &str) -> Result<[u8; 32]> {
        Ok(aithos_core::derive::derive_key(
            &format!("aithos-core/v1/x/{connector}"),
            &self.vault_dk(owner)?,
        ))
    }

    /// Grant an agent the vault line it needs to seal audit args (§07.9.3).
    pub fn grant_audit_line(
        &mut self,
        owner: &OwnerKeys,
        agent_pub: &ed25519_dalek::VerifyingKey,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let dk = self.vault_dk(owner)?;
        let mb = aithos_core::wire::ed25519_pub_to_multibase(&agent_pub.to_bytes());
        let recipient = aithos_core::header::Recipient {
            to: mb.clone(),
            kid: mb,
            pubkey: aithos_core::keys::ed2x(agent_pub),
        };
        let mut header: aithos_core::header::Header = self.get_json("e/x/header.json")?;
        header.append_line(&self.did.clone(), KV, &dk, &recipient, ent.e32(), ent.e24())?;
        self.put_json("e/x/header.json", &header)
    }

    /// A metered LLM call (§07.9.1): light clear entry, budget-checked like
    /// an action, never carrying prompt or response text.
    pub fn log_inference(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        spec: &InferenceSpec<'_>,
        ent: &mut dyn EntropySource,
    ) -> Result<Entry> {
        let doc = self.did_doc()?;
        let entries = self.gamma_entries()?;
        let via: Vec<String> = chain.iter().map(|m| m.id.clone()).collect();
        let mut payload = serde_json::json!({
            "provider": spec.provider,
            "model": spec.model,
            "tokens_in": spec.tokens_in,
            "tokens_out": spec.tokens_out,
        });
        if let Some(r) = spec.budget_ref {
            payload["budget_ref"] = serde_json::json!(r);
        }
        let entry = delegated_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: gamma::head(&entries)?,
                at: spec.now.to_owned(),
                kind: Kind::Inference,
                target: Some("x.llm".to_owned()),
                payload: Some(payload),
                body_enc: None,
            },
            via,
            agent_sk,
        )?;
        verify_delegated_entry(&entry, chain, &doc)?;
        check_action_append(&entries, &entry, chain, &doc)?;
        self.gamma_append(&entry)?;
        Ok(entry)
    }

    /// A journalized read (§07.9.2, `log_reads`): sealed body naming the
    /// section, under the very key that just opened it.
    pub fn log_read_as_agent(
        &mut self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        zone: Zone,
        display_path: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<Entry> {
        let doc = self.did_doc()?;
        let entries = self.gamma_entries()?;
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidGammaEntry("empty chain".to_owned()))?;
        let (row, folders) = self.resolve_clear(zone, display_path)?;
        let node = NodePath::section(zone, folders, Sid::parse(&row.sid)?);
        let kex = grantee_kex_secret(agent_sk);
        let key = self.agent_node_key(&leaf.grantee.pubkey, &kex, &node)?;
        let body = aithos_core::gamma::seal_body(
            &key,
            &self.did,
            &node.to_string(),
            KV,
            &serde_json::json!({}),
            &ent.e24(),
        )?;
        let via: Vec<String> = chain.iter().map(|m| m.id.clone()).collect();
        let entry = delegated_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: gamma::head(&entries)?,
                at: now.to_owned(),
                kind: Kind::EthosRead,
                target: None,
                payload: None,
                body_enc: Some(body),
            },
            via,
            agent_sk,
        )?;
        verify_delegated_entry(&entry, chain, &doc)?;
        self.gamma_append(&entry)?;
        Ok(entry)
    }

    /// A delegated minting, logged under the minting (parent) chain (§07.4):
    /// `max_children` gates it, and the logged entry is what makes the
    /// child's future chain presentations alive.
    pub fn log_delegated_grant(
        &mut self,
        minting_chain: &[Mandate],
        minter_sk: &SigningKey,
        child_mandate_id: &str,
        now: &str,
        ent: &mut dyn EntropySource,
    ) -> Result<()> {
        let doc = self.did_doc()?;
        let entries = self.gamma_entries()?;
        let minting = minting_chain
            .last()
            .ok_or_else(|| Error::InvalidGammaEntry("empty minting chain".to_owned()))?;
        check_grant_append(&entries, minting)?;
        let via: Vec<String> = minting_chain.iter().map(|m| m.id.clone()).collect();
        let entry = delegated_entry(
            EntrySpec {
                id: self.next_gamma_id(ent),
                prev: gamma::head(&entries)?,
                at: now.to_owned(),
                kind: Kind::Grant,
                target: Some(child_mandate_id.to_owned()),
                payload: Some(serde_json::json!({})),
                body_enc: None,
            },
            via,
            minter_sk,
        )?;
        verify_delegated_entry(&entry, minting_chain, &doc)?;
        self.gamma_append(&entry)
    }

    // ---------------------------------------------------------- verifying

    /// Full offline log verification: link integrity + every signature
    /// (owner entries under the DID keys, delegated entries against their
    /// stored certificate chains).
    pub fn gamma_verify(&self) -> Result<()> {
        let doc = self.did_doc()?;
        let entries = self.gamma_entries()?;
        verify_links(&entries)?;
        for e in &entries {
            match &e.authorized_via {
                None => gamma::verify_owner_entry(e, &doc)?,
                Some(via) => {
                    let chain: Vec<Mandate> = via
                        .iter()
                        .map(|id| self.get_json(&format!("certs/{id}.json")))
                        .collect::<Result<_>>()?;
                    verify_delegated_entry(e, &chain, &doc)?;
                }
            }
        }
        Ok(())
    }

    // ------------------------------------------------------------ reading

    /// Candidate section nodes of the clear zones, from the index.
    fn clear_zone_nodes(&self, zone: Zone) -> Result<Vec<NodePath>> {
        let index: ZoneIndex = self.get_json(&format!("e/{}/index.json", zone.as_str()))?;
        let by_sid: BTreeMap<&str, (&str, &Option<String>)> = index
            .folders
            .iter()
            .map(|f| (f.sid.as_str(), (f.sid.as_str(), &f.parent_sid)))
            .collect();
        let mut out = Vec::new();
        for row in &index.sections {
            let mut chain_rev = Vec::new();
            let mut cursor = row.folder_sid.clone();
            while let Some(sid) = cursor {
                let (s, parent) = by_sid
                    .get(sid.as_str())
                    .ok_or_else(|| Error::InvalidPath(format!("dangling folder {sid}")))?;
                chain_rev.push(Sid::parse(s)?);
                cursor = (*parent).clone();
            }
            chain_rev.reverse();
            out.push(NodePath::section(zone, chain_rev, Sid::parse(&row.sid)?));
        }
        Ok(out)
    }

    /// hint → (node, key) map for every node a key reaches, owner side.
    fn owner_hint_map(&self, owner: &OwnerKeys) -> Result<BTreeMap<String, (NodePath, [u8; 32])>> {
        let mut map = BTreeMap::new();
        for zone in [Zone::Circle, Zone::Self_] {
            let zone_dk = self.zone_dk(zone, owner)?;
            let nodes = match zone {
                Zone::Self_ => self.self_section_nodes(owner)?,
                _ => self.clear_zone_nodes(zone)?,
            };
            for node in nodes {
                let key = node_key(&zone_dk, &node);
                map.insert(gamma::body_hint(&key), (node, key));
            }
        }
        Ok(map)
    }

    /// Does an opened target satisfy the tree dimensions of the filter?
    fn target_matches(
        &self,
        target: &str,
        dir: Option<&(Zone, String)>,
        tag: Option<&str>,
    ) -> Result<bool> {
        if !target.starts_with("/e/") {
            // Connector-plane target (x.<c>): tree filters cannot match it.
            return Ok(dir.is_none() && tag.is_none());
        }
        let node = NodePath::parse(target)?;
        if let Some((zone, display)) = dir {
            let want = self.resolve_folder(*zone, display)?;
            let ok = node.zone == *zone
                && node.folders.len() >= want.len()
                && node.folders[..want.len()] == want[..];
            if !ok {
                return Ok(false);
            }
        }
        if let Some(t) = tag {
            let Leaf::Section(sid) = &node.leaf else {
                return Ok(false);
            };
            match node.zone {
                Zone::Self_ => return Ok(false), // sealed tags: owner filters post-open
                _ => {
                    let index: ZoneIndex =
                        self.get_json(&format!("e/{}/index.json", node.zone.as_str()))?;
                    let row = index.sections.iter().find(|r| r.sid == sid.to_string());
                    if !row.is_some_and(|r| r.tags.iter().any(|x| x == t)) {
                        return Ok(false);
                    }
                }
            }
        }
        Ok(true)
    }

    fn clear_dims_match(e: &Entry, f: &LogFilter) -> bool {
        f.kind.as_ref().is_none_or(|k| {
            &e.kind == k || aithos_core::gamma::Kind::parse(&e.kind).is_ok_and(|x| x.class() == k)
        }) && f.action.as_ref().is_none_or(|a| {
            e.payload
                .as_ref()
                .and_then(|p| p.get("action"))
                .and_then(|x| x.as_str())
                == Some(a.as_str())
        }) && f.since.as_ref().is_none_or(|s| e.at.as_str() >= s.as_str())
            && f.until.as_ref().is_none_or(|u| e.at.as_str() <= u.as_str())
            && f.mandate.as_ref().is_none_or(|m| {
                e.authorized_via
                    .as_ref()
                    .is_some_and(|v| v.iter().any(|id| id == m))
            })
    }

    /// Owner query (§07.8): scan the touched segments, open every body S
    /// reaches, filter dimension by dimension.
    pub fn log_query_owner(&self, owner: &OwnerKeys, filter: &LogFilter) -> Result<Vec<LogHit>> {
        let mut hints = self.owner_hint_map(owner)?;
        let entries = self.gamma_entries()?;
        // Audit keys for sealed action args (§07.9.3): connectors are clear
        // in the entries' targets.
        for e in entries.iter().filter(|e| e.body_enc.is_some()) {
            if let Some(c) = e.target.as_deref().and_then(|t| t.strip_prefix("x.")) {
                let key = self.audit_key_owner(owner, c)?;
                hints.insert(
                    gamma::body_hint(&key),
                    (NodePath::zone_root(Zone::Circle), key),
                );
            }
        }
        let mut out = Vec::new();
        for e in entries {
            if !Self::clear_dims_match(&e, filter) {
                continue;
            }
            let mut body = None;
            if let Some(enc) = &e.body_enc {
                let Some((node, key)) = hints.get(&enc.hint) else {
                    continue; // unreadable body cannot prove it matches — skip
                };
                let candidate = match e.target.as_deref() {
                    Some(t) if t.starts_with("x.") => t.to_owned(), // audit body
                    _ => node.to_string(),
                };
                let opened = open_body(key, &self.did, &candidate, KV, enc)?;
                if !self.target_matches(
                    &opened.target,
                    filter.zone_dir.as_ref(),
                    filter.tag.as_deref(),
                )? {
                    continue;
                }
                body = Some(opened);
            } else if (filter.zone_dir.is_some() || filter.tag.is_some())
                && !e.target.as_deref().is_some_and(|t| t.starts_with("/e/"))
            {
                continue; // tree filter on a non-tree entry
            } else if let Some(t) = e.target.as_deref() {
                if t.starts_with("/e/")
                    && !self.target_matches(t, filter.zone_dir.as_ref(), filter.tag.as_deref())?
                {
                    continue;
                }
            }
            out.push(LogHit { entry: e, body });
        }
        Ok(out)
    }

    /// Agent query (§07.8): the certificate half refuses out-of-perimeter
    /// queries; the physics half opens only what the agent's keys reach.
    pub fn log_query_as_agent(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        query: &GammaQuery,
        filter: &LogFilter,
        at: &str,
    ) -> Result<Vec<LogHit>> {
        let doc = self.did_doc()?;
        aithos_core::mandate::verify_chain(chain, &doc, at)?;
        let leaf = chain.last().expect("non-empty chain");
        if !covers_gamma_query(&leaf.parsed_perimeter()?, query) {
            return Err(Error::InvalidMandate(format!(
                "{}: gamma query exceeds the granted perimeter",
                leaf.id
            )));
        }
        let kid = leaf.grantee.pubkey.clone();
        let kex = grantee_kex_secret(agent_sk);
        // Physics: hint map over every node the agent's lines reach.
        // (`self` bodies stay owner-side until agents walk descriptors.)
        let mut hints: BTreeMap<String, (NodePath, [u8; 32])> = BTreeMap::new();
        for node in self.clear_zone_nodes(Zone::Circle)? {
            if let Ok(key) = self.agent_node_key(&kid, &kex, &node) {
                hints.insert(gamma::body_hint(&key), (node, key));
            }
        }
        let mut out = Vec::new();
        for e in self.gamma_entries()? {
            if !Self::clear_dims_match(&e, filter) {
                continue;
            }
            let mut body = None;
            if let Some(enc) = &e.body_enc {
                let Some((node, key)) = hints.get(&enc.hint) else {
                    continue;
                };
                let opened = open_body(key, &self.did, &node.to_string(), KV, enc)?;
                if !self.target_matches(
                    &opened.target,
                    filter.zone_dir.as_ref(),
                    filter.tag.as_deref(),
                )? {
                    continue;
                }
                body = Some(opened);
            }
            out.push(LogHit { entry: e, body });
        }
        Ok(out)
    }

    /// A single sealed entry read attempt with the agent's keys — the
    /// physics check the privacy scenarios exercise.
    pub fn open_entry_as_agent(
        &self,
        chain: &[Mandate],
        agent_sk: &SigningKey,
        entry: &Entry,
    ) -> Result<Body> {
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidGammaEntry("empty chain".to_owned()))?;
        let kid = leaf.grantee.pubkey.clone();
        let kex = grantee_kex_secret(agent_sk);
        let enc = entry
            .body_enc
            .as_ref()
            .ok_or_else(|| Error::InvalidGammaEntry(format!("{}: clear entry", entry.id)))?;
        for node in self.clear_zone_nodes(Zone::Circle)? {
            if let Ok(key) = self.agent_node_key(&kid, &kex, &node) {
                if gamma::body_hint(&key) == enc.hint {
                    return open_body(&key, &self.did, &node.to_string(), KV, enc);
                }
            }
        }
        Err(Error::SealRejected(format!(
            "{}: no key reaches this body",
            entry.id
        )))
    }

    /// Open and check one action's sealed args (§07.9.3): the args must
    /// reopen under the connector's audit key AND recompute to the entry's
    /// clear `args_hash` — a mismatch is a tampered audit trail.
    pub fn audit_action_args(&self, owner: &OwnerKeys, entry: &Entry) -> Result<serde_json::Value> {
        let err = |m: &str| Error::InvalidGammaEntry(format!("{}: audit: {m}", entry.id));
        let connector = entry
            .target
            .as_deref()
            .and_then(|t| t.strip_prefix("x."))
            .ok_or_else(|| err("not a connector action"))?;
        let enc = entry
            .body_enc
            .as_ref()
            .ok_or_else(|| err("no sealed args"))?;
        let key = self.audit_key_owner(owner, connector)?;
        let body = open_body(&key, &self.did, &format!("x.{connector}"), KV, enc)?;
        let canon = aithos_core::jcs::canonical_bytes(&body.payload)?;
        let recomputed = format!("sha256:{}", aithos_core::gamma::sha256_hex(&canon));
        let clear = entry
            .payload
            .as_ref()
            .and_then(|p| p.get("args_hash"))
            .and_then(|h| h.as_str())
            .ok_or_else(|| err("no clear args_hash"))?;
        if recomputed != clear {
            return Err(err("sealed args do not match the clear args_hash"));
        }
        Ok(body.payload)
    }

    /// Audit every sealed-args action on the log against a mandate's
    /// `action_params` predicates (§04.4 tier-V audit half).
    pub fn audit_log_against(&self, owner: &OwnerKeys, mandate: &Mandate) -> Result<usize> {
        let mut checked = 0;
        for e in self.gamma_entries()? {
            if e.kind != "action" || e.body_enc.is_none() {
                continue;
            }
            let args = self.audit_action_args(owner, &e)?;
            let action = e
                .payload
                .as_ref()
                .and_then(|p| p.get("action"))
                .and_then(|a| a.as_str())
                .unwrap_or_default()
                .to_owned();
            aithos_core::constraints::check_action_params(&mandate.constraints, &action, &args)?;
            checked += 1;
        }
        Ok(checked)
    }

    /// Action-covering check exposed for callers building presentations.
    pub fn action_covered(&self, chain: &[Mandate], connector: &str, action: &str) -> Result<bool> {
        let leaf = chain
            .last()
            .ok_or_else(|| Error::InvalidGammaEntry("empty chain".to_owned()))?;
        Ok(covers_act(
            &leaf.parsed_perimeter()?,
            &ActOp {
                connector: connector.to_owned(),
                action: action.to_owned(),
            },
        ))
    }
}
