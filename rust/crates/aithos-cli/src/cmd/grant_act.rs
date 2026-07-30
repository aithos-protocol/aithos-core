//! `aithos grant-act` — grant an agent CONNECTOR ACTION rights
//! (certificate only — actions need no content keys).

use aithos_bundle::entropy::OsEntropy;

use super::common::{bundle_at, getrandom, now_secs, now_string, owner_from, resolved_dir, ts};

#[derive(clap::Args)]
pub struct Args {
    #[arg(long)]
    pub dir: Option<String>,
    #[arg(long)]
    pub seed_hex: Option<String>,
    #[arg(long)]
    pub agent_seed_hex: String,
    #[arg(long, default_value = "agent")]
    pub label: String,
    /// Connector name, e.g. gmail → perimeter act.x.gmail.<action>
    pub connector: String,
    /// Action pattern (an action name, or * for the read/act class).
    #[arg(default_value = "*")]
    pub action_pat: String,
    #[arg(long, default_value_t = 7)]
    pub ttl_days: u32,
    /// Lifetime action budget (spec 04.4, counted by gamma 07.4).
    #[arg(long)]
    pub max_actions: Option<u64>,
    /// Dead-man heartbeat, e.g. --heartbeat-every 30d --heartbeat-grace 72h
    #[arg(long)]
    pub heartbeat_every: Option<String>,
    #[arg(long)]
    pub heartbeat_grace: Option<String>,
    /// Budget profiles as raw JSON (spec 04.11), e.g.
    /// '[{"id":"gemma","models":["gemma"],"token_budget":25000}]'
    #[arg(long)]
    pub budgets_json: Option<String>,
    /// Absolute active windows as raw JSON (spec 04.10), e.g.
    /// '[{"anchor":"2026-07-02T14:00:00Z","duration":"4h","period":"7d"}]'
    #[arg(long)]
    pub windows_json: Option<String>,
    /// Also grant the vault audit line so the agent can seal its action
    /// arguments (spec 07.9.3).
    #[arg(long, default_value_t = false)]
    pub audit: bool,
    /// Obligations as raw JSON (spec 04.12), e.g. '[{"id":"approval",
    /// "check":"human.approve","attestor":["z6Mk…"],"applies_to":
    /// "act.x.social.publish","verdict":"approve","max_age":"5m"}]'
    #[arg(long)]
    pub obligations_json: Option<String>,
    /// Action(s) requiring a fresh owner co-signature (spec 04.6,
    /// desugars to the reserved co_sign obligation). Repeatable.
    #[arg(long = "counter-sign")]
    pub counter_sign: Vec<String>,
}

pub fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let Args {
        dir,
        seed_hex,
        agent_seed_hex,
        label,
        connector,
        action_pat,
        ttl_days,
        max_actions,
        heartbeat_every,
        heartbeat_grace,
        budgets_json,
        windows_json,
        audit,
        obligations_json,
        counter_sign,
    } = args;
    let dir = resolved_dir(&dir)?;
    let owner = owner_from(&seed_hex)?;
    let agent = ed25519_dalek::SigningKey::from_bytes(
        &<[u8; 32]>::try_from(hex::decode(agent_seed_hex)?)
            .map_err(|_| "agent-seed-hex: 32 bytes")?,
    );
    let start = now_secs();
    let (nb, na) = (ts(start), ts(start + u64::from(ttl_days) * 86_400));
    let mut bundle = bundle_at(&dir)?;
    let mut constraints = serde_json::Map::new();
    if let Some(n) = max_actions {
        constraints.insert("max_actions".into(), n.into());
    }
    if let (Some(every), Some(grace)) = (&heartbeat_every, &heartbeat_grace) {
        constraints.insert(
            "heartbeat".into(),
            serde_json::json!({"every": every, "grace": grace}),
        );
    }
    if let Some(b) = &budgets_json {
        constraints.insert("budgets".into(), serde_json::from_str(b)?);
    }
    if let Some(wjs) = &windows_json {
        constraints.insert("active_windows".into(), serde_json::from_str(wjs)?);
    }
    if let Some(objs) = &obligations_json {
        constraints.insert("obligations".into(), serde_json::from_str(objs)?);
    }
    if !counter_sign.is_empty() {
        constraints.insert("counter_sign".into(), serde_json::json!(counter_sign));
    }
    let mut nonce = [0u8; 16];
    getrandom(&mut nonce)?;
    let mut id_bytes = [0u8; 16];
    getrandom(&mut id_bytes)?;
    let m = aithos_core::mandate::Mandate::build_root(
        &owner.root_sign,
        &aithos_core::mandate::MandateSpec {
            id: format!(
                "mandate_{}",
                aithos_core::ids::Sid(ulid::Ulid::from(u128::from_be_bytes(id_bytes)))
            ),
            subject: bundle.did.clone(),
            grantee_id: format!("urn:aithos:agent:{label}"),
            grantee_label: label,
            grantee_pub: &agent.verifying_key(),
            perimeter: vec![aithos_core::mandate::PerimeterEntry::parse(&format!(
                "act.x.{connector}.{action_pat}"
            ))?],
            constraints: serde_json::Value::Object(constraints),
            not_before: nb,
            not_after: na,
            issued_at: ts(start),
            nonce: hex::encode(nonce),
        },
    )?;
    std::fs::create_dir_all(format!("{dir}/certs"))?;
    std::fs::write(
        format!("{dir}/certs/{}.json", m.id),
        serde_json::to_vec_pretty(&m)?,
    )?;
    bundle.log_owner_grant(&owner, &m.id, &now_string(), &mut OsEntropy)?;
    if audit {
        bundle.grant_audit_line(&owner, &agent.verifying_key(), &mut OsEntropy)?;
        println!("audit line granted (sealed args enabled)");
    }
    println!("granted; cert = certs/{}.json", m.id);
    Ok(())
}
