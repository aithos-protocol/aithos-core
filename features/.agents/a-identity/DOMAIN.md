# Domaine — `a-identity.feature`

## Contrat

La feature couvre la genèse de l'identité Aithos :

- déterminisme depuis un master seed owner de 32 octets ;
- séparation des clés root, content et kex ;
- indépendance de la succession ;
- publication et vérification du document DID ;
- transition vers une nouvelle identité sous autorité de succession.

L'audit public est `docs/audits/features/a-identity.md`.

## Invariants protocolaires

1. Le DID est lié à la clé root et signé par elle.
2. Root, content et succession utilisent le codec Ed25519 attendu.
3. Kex utilise le codec X25519 attendu.
4. La version, l'algorithme et le fragment de signature sont fermés.
5. Les membres wire inconnus ne doivent pas être supprimés avant vérification.
6. Une transition d'époque doit lier le document précédent, la déclaration et
   le document successeur effectivement présenté.
7. Les identités précédente et suivante doivent être distinctes.
8. La succession ne doit pas être dérivable depuis le master owner.
9. La garde froide doit être une propriété testable des surfaces qui la
   revendiquent.

## Sources principales

| Objet | Chemin |
|---|---|
| Contrat | `features/a-identity.feature` |
| Steps | `rust/crates/aithos-bundle/tests/cucumber.rs` |
| Clés | `rust/crates/aithos-core/src/keys.rs` |
| DID et transition | `rust/crates/aithos-core/src/did.rs` |
| Dérivation et wire | `rust/crates/aithos-core/src/{derive,wire}.rs` |
| Consommation Bundle | `rust/crates/aithos-bundle/src/bundle.rs` |
| Création Gateway | `rust/crates/aithos-gateway/src/core_bridge.rs` |
| Custody CLI | `rust/crates/aithos-cli/src/{main,custody}.rs` |
| Dépôt Provider | `rust/crates/aithos-provider/src/artifacts.rs` |
| Tests Core | `rust/crates/aithos-core/tests/{a1_genesis,a2_did}.rs` |
| Vecteurs | `vectors/{a1-genesis,a2-did}.json` |

Après le correctif AID-001/002/005, contrôler aussi
`rust/crates/aithos-bundle/tests/aid_identity_surfaces.rs`.

## Gates minimaux

```text
cargo test -p aithos-core --test a1_genesis --test a2_did
cargo test -p aithos-bundle --test aid_identity_surfaces
cargo test -p aithos-bundle --test cucumber
cargo test --workspace --no-fail-fast
cargo fmt --all -- --check
```

Si un test n'existe pas sur la baseline examinée, le signaler au lieu de
transformer son absence en succès.

Le runner Cucumber parcourt toutes les features non `@wip`. Contrôler dans sa
sortie le nombre exact de scénarios de `a-identity`, pas seulement son code de
sortie global.

## Surfaces et voisinages à inspecter

- Bundle : parsing de `did.json` et cold verification ;
- WASM/client : vérification publique des mandats et DID ;
- Gateway : création de l'identité et source de succession ;
- Provider : remplacement et distribution de `did.json` ;
- `f-gamma.feature` : faits `rotate identity`, distincts de la transition
  d'époque mais susceptibles de partager les invariants DID.

## Limites du pilote

Auditer uniquement la vérité des scénarios existants et des tests ajoutés pour
fermer AID-001, AID-002 et AID-005. Ne pas concevoir de nouveaux scénarios
généraux. AID-003 et AID-004 restent ouverts tant qu'ils ne sont pas assignés
explicitement à un round de correction.
