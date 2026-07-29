# Conclusion — review indépendante Identity, round 1

| Champ | Valeur |
|---|---|
| Type | `REVIEW` |
| Rôle | auditeur `audit-a-identity` |
| Date | 2026-07-29 |
| Branche de review | `codex/review-a-identity` |
| HEAD observé | `0601b9f9106988385c2b38ed9d4a2e2370ab728a` |
| Baseline d'audit | `be2d098eeb79107c861462a6433df9ef45871265` |
| Commit candidat | `56436f33d427dbaf5f55813ed0febb981ea43dca` |
| Client frère inspecté | `c6f615123ca3dc83708ba029b898375409551719` |
| État initial du worktree | propre |
| Résultat | `DECISION_REQUIRED` |
| Prérequis bloquant | sémantique du remplacement Provider `did.json` |

## Verdict

| Finding | Verdict de review | Motif |
|---|---|---|
| `AID-001` | `DÉCISION PROTOCOLAIRE REQUISE` | Core et les surfaces ordinaires sont durcis. Le remplacement Provider `artifacts::deposit_did` conserve une sémantique même-DID distincte ; décider si elle reste spécifique ou adopte §10.4 relève du propriétaire du protocole, pas du correcteur. |
| `AID-002` | `VÉRIFIÉ` | Le triplet précédent/transition/successeur est réellement reçu et validé ; les liaisons, signatures, métadonnées et identités distinctes sont couvertes. |
| `AID-005` | `VÉRIFIÉ DANS LE PÉRIMÈTRE DU PILOTE` | Les 21 scénarios ajoutés sont honnêtes, sélectionnés et verts. La cérémonie dépend d'AID-003/AID-004 hors round ; les vecteurs indépendants et le gate automatisé de comptage sont des améliorations, pas des preuves indispensables à ce pilote. |
| `AID-003` | non traité | Hors correction du round 1 ; reste ouvert. |
| `AID-004` | non traité | Hors correction du round 1 ; reste ouvert. |

## Diff revu

Le diff exact `be2d098..56436f3` contient 7 fichiers, 1130 insertions et
158 suppressions :

- `rust/crates/aithos-core/src/did.rs` :
  `DidDocument::verify`, `EpochTransition::{verify_declaration,
  verify_succession}`, fermeture serde et constantes de signature ;
- `rust/crates/aithos-core/tests/a2_did.rs` :
  trois tests AID-001/AID-002 supplémentaires ;
- `rust/crates/aithos-bundle/tests/cucumber.rs` :
  steps Identity et verdicts propres à chaque scénario ;
- `rust/crates/aithos-bundle/tests/aid_identity_surfaces.rs` :
  rejeu Bundle et chaîne de mandats/WASM ;
- `features/a-identity.feature` : 21 exemples/scénarios supplémentaires ;
- `docs/audits/features/{README.md,a-identity.md}` : documentation de
  correction.

`git diff --check be2d098..56436f3` est propre.

## AID-001

### Preuves acceptées

`DidDocument::verify` contrôle maintenant :

- `DID_VERSION`, `ed25519` et `#root` ;
- root, content et succession sous codec Ed25519 avec construction de
  `VerifyingKey` ;
- kex sous codec X25519 ;
- la liaison `id ↔ root` et la signature root ;
- le refus des membres wire inconnus sur `DidDocument`, `DidKeys` et
  `SignatureBlock`.

Les chemins suivants rejoignent ce verdict :

- `Bundle::open` et `Bundle::verify` ;
- WASM `verify_mandate_chain` via `mandate::verify_chain` ;
- Catalog `verified_owner_did` ;
- Gateway, notamment `Bundle::open` et les snapshots de contrôle ;
- le client frère `c6f6151`, dont les chargements DID appellent
  `DidDocument::verify`.

### Décision protocolaire requise

Le remplacement Provider de `did.json` reste parallèle :

- `artifacts::deposit_did` accepte le remplacement sous la succession du
  document stocké et n'appelle pas `doc.verify()` ;
- il ne contrôle pas `doc.version` ni `doc.keys.kex` ;
- il décode root/content/succession mais ne construit pas leurs
  `VerifyingKey` ;
- le fixture P9 `did_rotation_ok` confirme que ce document `#succession` est
  persisté alors que le verdict Core exige `#root`.

Cette surface peut donc commettre durablement un objet que les consommateurs
Core/Bundle refuseront à la réouverture. Cependant, P9 codifie une succession
même-DID distincte de la transition d'époque §10.4. Choisir entre ces deux
sémantiques est une décision de protocole : la partie Core d'AID-001 est
acceptée, mais aucune correction Provider ne doit être demandée avant cet
arbitrage explicite.

## AID-002

`EpochTransition::verify_succession(prev_doc, next_doc)` :

- appelle le validateur strict sur les deux documents ;
- valide version, algorithme et `#succession` ;
- lie `prev_did` et `next_did` aux documents présentés ;
- refuse les identités identiques ;
- vérifie la signature sous la succession précédente.

Le step « transition is signed by the succession key » transmet désormais
`next_doc`. Les 10 défauts de l'Outline et le cas root prétendant
`#succession` construisent chacun leur transition/document puis consomment
leur propre résultat.

La sémantique Provider « remplacement sous le même DID » reste nommée comme
distincte et ne prétend pas implémenter §10.4. Aucun appelant de production
n'utilisait l'ancienne `EpochTransition::verify`.

Verdict : `AID-002` passe à `VÉRIFIÉ`.

## AID-005

Les éléments livrés sont réels :

- le scénario d'altération post-signature est nommé précisément ;
- 7 documents correctement re-signés mais invalides ;
- 3 membres wire inconnus ;
- le `Then` positif vérifie le triplet complet ;
- 10 transitions mal liées et 1 signature root prétendant la succession ;
- aucun step Identity n'est vide, proxy, `@wip` ou adossé à un `OnceLock`.

Les éléments de l'audit initial encore absents se répartissent ainsi :

1. un scénario de cérémonie passant par la surface réelle de création
   d'identité : dépendance AID-003/AID-004, explicitement hors round ;
2. des négatifs A2 générés indépendamment : amélioration de robustesse ;
3. un gate ciblé échouant si le nombre exécuté diffère de 30 : amélioration
   d'outillage. Le processus demande ici le comptage effectif par l'auditeur,
   qui a été réalisé.

Le correcteur rapporte en outre 3 tests A2 et 18 scénarios RED au moyen de
shims temporaires. Ces shims ne sont pas versionnés ; l'auditeur n'a pas
modifié le Rust pour reconstruire ces nombres. Le diff de baseline prouve
statiquement les anciens défauts, mais pas ces comptes exacts. Cette limite est
rapportée et ne bloque pas la vérité des scénarios corrigés ni leurs gates
GREEN.

Verdict : `AID-005` passe à `VÉRIFIÉ DANS LE PÉRIMÈTRE DU PILOTE`.

## Commandes réellement exécutées

### Limite du worktree direct

```text
cargo test -p aithos-core --test a1_genesis --test a2_did
EXIT=101
package collision in the lockfile:
aithos-bundle du worktree de review et aithos-bundle du worktree principal
```

Le client frère `c6f6151` référence `../aithos-core`. Pour tester le commit
immuable sans modifier aucun fichier Rust/Cargo, des archives Git exactes de
`56436f3` et `c6f6151` ont été extraites sous un layout frère temporaire.

### Gates ciblés sur les archives exactes

```text
cargo test -p aithos-core --test a1_genesis --test a2_did
EXIT=0
a1_genesis: 4 passed
a2_did:     6 passed

cargo test -p aithos-bundle --test aid_identity_surfaces
EXIT=0
2 passed

cargo test -p aithos-bundle --test cucumber
EXIT=0
18 features
114 rules
836 scenarios (836 passed)
3568 steps (3568 passed)
```

La sortie énumère les 30 scénarios Identity et leurs 93 steps, tous passés.

### Gate workspace

```text
cargo test --workspace --no-fail-fast
EXIT=101
28 targets failed
```

Les cibles Identity sont vertes dans ce run. Les 28 cibles échouent lorsqu'un
test CLI/Gateway/Provider tente d'ouvrir une socket ou un service local :
`Operation not permitted`. Une relance hors sandbox a été demandée et refusée
par la politique d'exécution. Le gate workspace est donc non concluant pour
raison environnementale ; il n'est pas présenté comme vert.

Le runner Provider non réseau a, lui, terminé vert avec 151/151 scénarios et
992/992 steps, dont le cas P9 `did_rotation_ok` utilisé dans le verdict
AID-001.

### Formatage

```text
cargo fmt --all -- --check
EXIT=1
rust/crates/aithos-gateway/src/core_bridge.rs:1355
```

Le blob `core_bridge.rs` est identique dans la baseline et le candidat
(`774672a0e2d4db1e866d3eb1d85106e53f684f80`). L'écart est préexistant et
hors du diff Identity.

## Limites

- Le workspace gate n'a pas pu être rejoué hors sandbox.
- Clippy n'a pas été rejoué par l'auditeur.
- Les nombres RED à shims restent rapportés, non reproduits.
- Aucun fichier Rust n'a été modifié.
- AID-003 et AID-004 n'ont pas été fermés, corrigés ou élargis.

## Handoff

Passer d'abord au propriétaire du protocole :

1. décider si le remplacement Provider `did.json` reste une succession
   même-DID spécifique ou adopte la transition d'époque §10.4 ;
2. après cette décision seulement, passer à `correct-a-identity`, round 2,
   baseline `56436f3`, si une correction est requise ;
3. conserver AID-002 et AID-005 inchangés et `VÉRIFIÉS` dans le périmètre du
   pilote ;
4. ne pas traiter AID-003/AID-004 dans ce round.
