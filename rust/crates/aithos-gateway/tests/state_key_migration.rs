//! SPL-2 — migration de la clé d'état du bridge (chantier split repo).
//!
//! Un store écrit avant la migration porte son état sous
//! `gateway/state.json` (l'ancienne clé nominative de la grammaire).
//! L'ouverture d'un contexte doit recopier les octets tels quels sous
//! `x/gateway/state.json`, servir le bridge normalement, et ne jamais
//! supprimer l'ancien objet.

use std::sync::Arc;

use aithos_gateway::core_bridge::{
    agent_pub_multibase, gateway_pub_multibase, owner_grant_context, owner_init_context, Bridge,
    MandateWindow, SeqEntropy, LEGACY_STATE_PATH, STATE_PATH,
};
use aithos_gateway::keyholder::Keyholder;
use aithos_gateway::store_adapter::GatewayStore;

const NOW: &str = "2026-07-16T12:00:00Z";

#[test]
fn opening_a_pre_migration_store_rewrites_the_state_under_the_new_key() {
    let master = [7u8; 32];
    let keyholder = Arc::new(Keyholder::from_entropy([0x31; 32], [0x41; 32]));
    let window = MandateWindow {
        not_before: "2026-07-16T11:00:00Z".to_owned(),
        not_after: "2026-07-16T12:01:00Z".to_owned(),
    };
    let root = tempfile::tempdir().unwrap();
    let store = GatewayStore::Fs(root.path().to_path_buf());
    let mut entropy = SeqEntropy::default();
    owner_init_context(&master, "company-brand", store.clone(), NOW, &mut entropy).unwrap();
    owner_grant_context(
        &master,
        "company-brand",
        &agent_pub_multibase(&keyholder),
        &gateway_pub_multibase(&keyholder),
        &["brand.read".to_owned()],
        store.clone(),
        &window,
        NOW,
        &mut entropy,
    )
    .unwrap();

    // L'équipement actuel écrit sous la clé migrée, et n'écrit jamais
    // l'ancienne.
    let new_path = root.path().join(STATE_PATH);
    let legacy_path = root.path().join(LEGACY_STATE_PATH);
    assert!(new_path.is_file(), "equip writes x/gateway/state.json");
    assert!(!legacy_path.exists(), "equip never writes the legacy key");

    // Simuler un store d'avant la migration : l'état ne vit que sous
    // l'ancienne clé.
    std::fs::create_dir_all(legacy_path.parent().unwrap()).unwrap();
    std::fs::rename(&new_path, &legacy_path).unwrap();
    let legacy_bytes = std::fs::read(&legacy_path).unwrap();

    // Ouverture d'un contexte : le bridge migre puis sert.
    let bridge = Bridge::open(store, keyholder, Box::new(SeqEntropy::default()))
        .expect("a pre-migration store still opens");
    drop(bridge);
    assert!(
        new_path.is_file(),
        "open rewrites the state under x/gateway/state.json"
    );
    assert_eq!(
        std::fs::read(&new_path).unwrap(),
        legacy_bytes,
        "the bytes are copied verbatim"
    );
    assert!(
        legacy_path.is_file(),
        "the legacy object is never deleted by the migration"
    );
}
