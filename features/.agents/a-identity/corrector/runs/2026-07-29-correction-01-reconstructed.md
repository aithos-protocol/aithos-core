# Conclusion reconstruite — correction Identity, round 1

| Champ | Valeur |
|---|---|
| Type | `RECONSTRUIT` |
| Rôle source | agent de correction externe |
| Date du commit | 2026-07-29 |
| Baseline | `be2d098` |
| Commit candidat | `56436f3` |
| Branche | `fix/aid-001-002-005-identity-fail-closed` |
| Findings annoncés | `AID-001`, `AID-002`, majeure partie de `AID-005` |
| Résultat | `REVIEW_REQUESTED` |

## Provenance

Cette conclusion a été reconstruite depuis le commit `56436f3`, son message,
son diff et les résultats écrits par le correcteur dans l'audit public. Elle
ne constitue pas une review indépendante et ne transforme aucun finding en
`VÉRIFIÉ`.

## Correctifs candidats observables

### AID-001

- validation explicite de la version DID et des métadonnées de signature ;
- validation des quatre clés avec leurs codecs attendus ;
- fermeture des schémas wire avec refus des membres inconnus ;
- cas négatifs correctement re-signés afin d'isoler la sémantique.

### AID-002

- séparation entre `verify_declaration(prev)` et
  `verify_succession(prev, next)` ;
- validation des deux documents DID ;
- liaison de `prev_did` et `next_did` aux documents présentés ;
- refus d'une transition vers la même identité ;
- modification du `Then` pour transmettre réellement le successeur.

### AID-005

- passage annoncé de 9 à 30 scénarios ;
- ajout de cas wire, documents correctement signés mais invalides et
  transitions mal liées ;
- ajout d'un test de surfaces Bundle/WASM ;
- conservation des vecteurs A2 positifs annoncée byte-identique.

## Diff

```text
7 fichiers modifiés
1130 insertions
158 suppressions

M docs/audits/features/README.md
M docs/audits/features/a-identity.md
M features/a-identity.feature
A rust/crates/aithos-bundle/tests/aid_identity_surfaces.rs
M rust/crates/aithos-bundle/tests/cucumber.rs
M rust/crates/aithos-core/src/did.rs
M rust/crates/aithos-core/tests/a2_did.rs
```

## Résultats rapportés par le correcteur

Ces résultats doivent être reproduits par l'auditeur :

```text
Avant correctif :
workspace 627 tests
cucumber bundle 815 scénarios

RED :
a2_did : 3 échecs sémantiques attendus
cucumber : 18 des 21 nouveaux scénarios en échec

Après correctif :
workspace 632 tests, 0 échec
cucumber bundle 836 scénarios, 0 échec
```

Le correcteur signale un écart `cargo fmt --check` préexistant dans
`aithos-gateway/src/core_bridge.rs`. Sa qualification doit être vérifiée sans
l'inclure silencieusement dans ce correctif.

## Hors périmètre déclaré

- `AID-003` : non traité ;
- `AID-004` : non traité ;
- aucune revendication de garde froide ;
- aucune validation indépendante de la conclusion présente.

## Handoff demandé

Lancer `audit-a-identity` en mode `review` sur le diff
`be2d098..56436f3`. Accepter ou refuser AID-001, AID-002 et AID-005
séparément. Ne promouvoir un finding à `VÉRIFIÉ` qu'après reproduction des
preuves et inspection des surfaces.
