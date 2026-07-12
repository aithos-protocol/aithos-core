# Handoff — Pass L : écritures déléguées (circle) + super-mandat

> État au 2026-07-12, session Cowork. Correctif développé et validé dans le
> sandbox cloud ; **PAS ENCORE ÉCRIT SUR LE DISQUE LOCAL** (bridge desktop
> déconnecté au moment du commit). C'est l'action n°1 ci-dessous.

## Contexte en deux phrases

Un agent gateway affirmait qu'on ne peut pas écrire une section sous mandat
sur circle/self. Verdict : la spec le permet depuis toujours (§04.2, §04.3,
§07.2) mais l'implémentation ne l'avait pas câblé — corrigé par cette passe
(rituel respecté : feature d'abord, 12 scénarios rouges, puis code jusqu'au
vert). Preuve complète et détail : `docs/2026-07-12-delegated-writes.md`.

## État des tests (dans le sandbox, après correctif)

- `cargo test -p aithos-bundle --test cucumber` : 14 features, 203/203
  scénarios, 826 steps.
- `cargo test --workspace` : toutes suites vertes (gateway cucumber 18/18).
- `cargo clippy --workspace --all-targets` : propre.

## Fichiers modifiés/créés (10)

| Fichier | Changement |
|---|---|
| `features/l-delegated-writes.feature` | NOUVEAU — 12 scénarios : écritures déléguées + super-mandat |
| `docs/2026-07-12-delegated-writes.md` | NOUVEAU — note de preuve pour l'agent gateway |
| `rust/crates/aithos-bundle/src/grants.rs` | `GrantSpec.verb`, `deliver_zone_line`, `agent_current_section_key`, `section_add/rewrite/delete_as_agent` |
| `rust/crates/aithos-bundle/src/log.rs` | `log_delegated_mutation` (body scellé, entrée déléguée) |
| `rust/crates/aithos-bundle/src/bundle.rs` | `new_sid` → `pub(crate)` (seule modif) |
| `rust/crates/aithos-bundle/tests/cucumber.rs` | steps L (fin de fichier), `verb_spec`, imports `Verb` |
| `rust/crates/aithos-bundle/benches/perf.rs` | `GrantSpec` + champ verb |
| `rust/crates/aithos-core/src/mandate.rs` | `Verb::parse`/`as_str` publics (seule modif) |
| `rust/crates/aithos-cli/src/main.rs` | `grant --verb read\|edit\|append\|delete\|write` |
| `rust/crates/aithos-gateway/src/core_bridge.rs` | `record_section_add/rewrite/delete` + `write_denied` |

## À faire pour finaliser

1. **Écrire les 10 fichiers sur le disque local** (`/Volumes/Math17/aithos/
   code/aithos-core/...`). Ils sont livrés dans la conversation Cowork du
   2026-07-12 ; une session avec le bridge connecté peut les committer, ou
   les télécharger depuis le chat. Puis relancer la suite EN LOCAL pour
   confirmer le vert chez toi (le sandbox l'a validé, pas ta machine).
2. **Git** : commit dédié pass L une fois le vert local confirmé.
3. **Gateway (pour l'agent gateway, son domaine)** : le tool-map
   d'onboarding ne mint que des périmètres `act.x.mcp.*` — brancher des
   périmètres d'écriture dans la config/onboarding (`config.rs`, `equip()`,
   `owner_grant_context`) pour que `record_section_*` soit utilisable en
   prod, avec ses scénarios dans `rust/crates/aithos-gateway/tests/features/`.
4. **Passe self (décidé : plus tard, passe dédiée)** : écriture déléguée sur
   `self` au niveau zone et par `id=` (les seuls modes que la spec autorise,
   §02.8) — les descripteurs scellés ajoutent la complexité ; noter que le
   code owner est lui-même « circle only this pass » pour rewrite/delete.
5. **Optionnel, trancher dans la spec** : une ligne au §07 pour dire
   explicitement qu'un « append gamma pur » sans acte couvert n'existe pas
   (kinds = registre fermé, chaque kind exige son autorité) — aujourd'hui
   c'est vrai par construction mais non écrit.

## Lancer les tests

```bash
cd /Volumes/Math17/aithos/code/aithos-core/rust

# La suite BDD complète (203 scénarios, dont les 12 de pass L à la fin)
cargo test -p aithos-bundle --test cucumber

# Tout le workspace (unit + intégration + gateway + CLI)
cargo test --workspace

# Lint
cargo clippy --workspace --all-targets
```

Chemins utiles : scénarios dans `features/*.feature` (pass L =
`features/l-delegated-writes.feature`), steps dans
`rust/crates/aithos-bundle/tests/cucumber.rs` (section
`// --- step L: delegated writes` en fin de fichier), tests gateway dans
`rust/crates/aithos-gateway/tests/`.

## Prompt de reprise (copier-coller dans un nouveau contexte)

> Contexte : aithos-core (`/Volumes/Math17/aithos/code/aithos-core`). La
> pass L (écritures déléguées circle + super-mandat) a été développée et
> validée en sandbox le 2026-07-12 — lis `docs/HANDOFF-2026-07-12-pass-L.md`
> et `docs/2026-07-12-delegated-writes.md` d'abord. Vérifie que les 10
> fichiers de la passe sont bien présents sur le disque (sinon ils sont dans
> la conversation Cowork du 12/07 — les committer d'abord). Ensuite, dans
> l'ordre : (1) relancer `cargo test --workspace` en local et confirmer
> 203/203 au cucumber bundle ; (2) commit git dédié ; (3) brancher les
> périmètres d'écriture dans l'onboarding gateway (config/equip) avec leurs
> scénarios gateway — rituel BDD : feature taguée @wip d'abord, puis code ;
> (4) préparer la passe self (écriture déléguée zone/id= uniquement, §02.8).
