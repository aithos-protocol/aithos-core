# HANDOFF — Correctifs ciblés après la revue d'impact `a-identity`

## Prompt de reprise

> Reprendre les suivis ciblés de la revue d'impact `a-identity` dans un
> contexte de correction indépendant. Lire intégralement
> `docs/HANDOFF-A-IDENTITY-IMPACT-FOLLOWUPS-2026-07-29.md` et toutes ses
> références obligatoires avant toute modification. État disque et Git =
> vérité. Travailler dans un worktree/une branche dédiée à partir de
> la révision courante de `main` qui contient ce handoff. Corriger uniquement
> `IMP-AID-01` et ajouter les régressions étroites `IMP-AID-02`. Ne pas
> modifier le comportement de production sauf si un test démontre un défaut ;
> dans ce cas, s'arrêter et documenter le besoin avant d'élargir le périmètre.
> Ne pas relancer ni auto-accepter un audit dans ce contexte. Terminer par un
> commit candidat, les résultats exacts des gates et un handoff vers une revue
> indépendante.

## Objectif

Fermer les deux suivis non bloquants identifiés après l'acceptation de la
correction Provider `AID-001` :

1. supprimer l'ambiguïté normative de `spec/10-threat-model.md` sur le
   signataire du document DID successeur ;
2. graver des régressions Gateway étroites autour de l'immuabilité de
   `did.json` pendant la réplication.

Ce lot ne conçoit pas le futur transport complet
`previous document / EpochTransition / successor document`. Ce transport reste
hors périmètre et fail-closed.

## État de départ immuable

| Élément | Valeur |
|---|---|
| Dépôt de référence | `/Volumes/Math17/aithos/v2/code/aithos-core-review-a-identity` |
| Branche de référence | `codex/review-a-identity` |
| Base de départ requise | la révision courante de `main` qui contient ce handoff, à résoudre et enregistrer avant le premier changement |
| Baseline de la correction Provider acceptée | `dfb79c87120caeb26737c81babd5cc2ad0dc0a3c` |
| Correction Provider acceptée | `e6fc5dc206204038e4bac80dcd9dc5f4c4429bc1` |
| Revue d'audit acceptée | `features/.agents/a-identity/auditor/runs/2026-07-29-audit-review-02.md` |
| Revue d'impact | `features/.agents/orchestrator/runs/2026-07-29-a-identity-impact-review.md` |
| Branche suggérée | `codex/fix-a-identity-impact-followups` |
| Worktree suggéré | `/Volumes/Math17/aithos/v2/code/aithos-core-fix-a-identity-impacts` |

Avant de créer le worktree, vérifier `git status`, `git worktree list` et
l'existence éventuelle de la branche ou du chemin suggérés. Ne pas déplacer,
nettoyer ni écraser les worktrees existants.

Le rapport d'impact peut être non suivi dans le worktree de référence. S'il
n'est pas présent dans le nouveau worktree, le lire à son chemin absolu
ci-dessus ; ne pas le recréer de mémoire.

## Références obligatoires

Lire intégralement, dans cet ordre :

1. `features/AGENTS.md`;
2. `features/.agents/PROCESS.md`;
3. `features/.agents/a-identity/DOMAIN.md`;
4. `features/.agents/a-identity/STATE.md`;
5. `features/.agents/a-identity/decisions/2026-07-29-aid-001-provider-epoch-transition.md`;
6. `features/.agents/a-identity/auditor/runs/2026-07-29-audit-review-02.md`;
7. `features/.agents/orchestrator/runs/2026-07-29-a-identity-impact-review.md`;
8. `spec/01-identity-and-keys.md`, en particulier §1.1 et §1.4 ;
9. `spec/04-mandates.md`, en particulier la sémantique de
   `transition_digest` ;
10. `spec/10-threat-model.md`, en particulier §10.4 ;
11. `rust/crates/aithos-gateway/src/store_adapter.rs`, en particulier
    `GatewayStore::replicate_now`, `replicate_paths` et
    `replicate_owner_history` ;
12. `rust/crates/aithos-gateway/tests/e2e_journal_remote.rs` ;
13. les six scénarios `@did` de
    `rust/crates/aithos-provider/tests/features/store/store-publication.feature`.

## Invariants déjà acceptés

Ne pas les redécider :

1. un document `did.json` est signé par sa propre clé `#root` ;
2. la clé froide `#succession` de l'identité précédente signe uniquement
   l'artefact `EpochTransition` ;
3. le successeur porte un DID distinct, dérivé de sa nouvelle clé root ;
4. le Provider refuse tout remplacement byte-différent sous le même
   `did.json` avec `artifact_invalid / immutable_conflict` ;
5. un redépôt byte-identique reste idempotent ;
6. aucune partie d'une future transition complète ne doit être persistée avant
   vérification du triplet et disponibilité d'un commit atomique cross-DID.

## Lot 1 — `IMP-AID-01`, clarification normative

### Défaut

`spec/10-threat-model.md:40-41` dit actuellement qu'un nouveau document DID est
« signed by the cold succession key ». Cette formulation contredit le format
fermé de `did.json` et la décision acceptée.

### Correction attendue

Modifier uniquement le passage utile de §10.4 pour exprimer sans ambiguïté :

- le nouveau document DID est signé par la nouvelle clé `#root` ;
- l'ancienne clé froide `#succession` autorise ce successeur en signant un
  artefact `EpochTransition` séparé ;
- l'acceptation porte sur la transition complète, pas sur un remplacement
  same-DID.

Conserver le sens opérationnel du paragraphe et ses renvois. Ne pas réécrire
les autres sections de la spécification.

### Preuve d'acceptation

Une recherche ciblée ne doit plus trouver de texte normatif affirmant qu'un
`did.json` est signé par `#succession`. Les archives historiques peuvent
conserver leur formulation si elles sont explicitement non normatives.

## Lot 2 — `IMP-AID-02`, régressions Gateway

### Surface

- `GatewayStore::replicate_now` appelle `replicate_paths`, qui re-PUT
  `did.json` pendant un sweep complet ;
- le Provider accepte des octets identiques et refuse des octets différents ;
- `replicate_owner_history` compare d'abord les valeurs JSON et refuse déjà un
  document distant différent avec `ErrorKind::AlreadyExists`.

### Preuves minimales à ajouter

Ajouter des tests ciblés sur les chemins de production réels, de préférence
dans `rust/crates/aithos-gateway/tests/e2e_journal_remote.rs` en réutilisant le
Provider local déjà présent :

1. **Sweep complet idempotent**  
   Deux sweeps complets successifs contenant le même `did.json` réussissent et
   la copie distante reste byte-identique.

2. **Conflit same-DID sans effet partiel**  
   Un document différent mais strict-Core-valide, root-signé et portant le
   même DID, envoyé par le chemin de réplication, produit
   `artifact_invalid / immutable_conflict`; une relecture indépendante prouve
   que les octets distants originaux n'ont pas changé.

3. **Refus client-side de l'historique propriétaire**  
   `replicate_owner_history` confronté à un document distant
   sémantiquement différent retourne `ErrorKind::AlreadyExists` avant tout PUT
   et ne modifie pas le Provider.

Les assertions doivent distinguer le code de registre `artifact_invalid` de
son `reason = immutable_conflict`; ne pas se contenter d'un `is_err()`.

Il est acceptable de répartir ces preuves entre un test E2E réel et un test
plus étroit si le seam existant l'impose, mais chaque assertion doit atteindre
le chemin de production qu'elle prétend couvrir. Ne pas ajouter de scénario
Gherkin général : la revue d'impact a recommandé des régressions
helper/E2E ciblées.

### Règle d'arrêt

Le comportement courant est présumé aligné. Si l'un des tests exige une
modification de `store_adapter.rs`, du client distant, du Provider, d'une API
ou d'un format pour passer :

1. conserver la preuve RED ;
2. ne pas corriger immédiatement le code de production ;
3. documenter le défaut, le chemin exact et l'élargissement proposé ;
4. demander une décision avant de poursuivre.

Cette barrière évite de transformer un lot de documentation/tests en
changement comportemental non audité.

## Hors périmètre

- modifier `a-identity.feature` ou ses marqueurs d'audit ;
- rouvrir `AID-001`, `AID-002` ou `AID-005` sans nouvelle preuve ;
- traiter `AID-003` ou `AID-004` ;
- concevoir le transport ou le stockage atomique du triplet d'époque ;
- refactoriser `RemoteStore`, `ObjectStore`, `PutOnce` ou les backends ;
- modifier P9, déjà vérifié à 58 checks / 32 cas ;
- corriger des défauts de formatage préexistants hors fichiers touchés ;
- lancer ou auto-accepter l'audit final dans le contexte correcteur.

## Gates obligatoires

Exécuter au minimum :

```text
git diff --check
cargo test -p aithos-gateway --lib store_adapter
cargo test -p aithos-gateway --test e2e_journal_remote
cargo test -p aithos-provider --test cucumber -- --tags @did
python3 vectors/verify-p9.py
cargo fmt --all -- --check
```

Notes :

- les tests E2E ouvrent une socket locale et peuvent nécessiter l'autorisation
  loopback de l'environnement ;
- une exécution antérieure de `cargo fmt --all -- --check` signalait
  `rust/crates/aithos-gateway/src/core_bridge.rs` avec le même blob avant et
  après la correction Provider. Si cela subsiste, prouver que ce défaut est
  préexistant et vérifier séparément les fichiers touchés ;
- ne pas lancer le workspace complet par défaut : la revue précédente a
  épuisé le volume temporaire pendant le link Provider/Gateway. Le lancer
  seulement si l'espace disponible et le besoin de preuve le justifient.

Pour chaque gate, enregistrer la commande exacte, le commit testé, le résultat
et le nombre de tests/scénarios lorsque disponible.

## Livrables du contexte correcteur

1. la clarification ciblée de `spec/10-threat-model.md` ;
2. les régressions Gateway ciblées ;
3. un diff sans modification hors périmètre ;
4. un commit candidat unique ou une petite série clairement délimitée ;
5. un résumé des gates exacts ;
6. les identifiants immuables `baseline..candidate` pour la revue suivante ;
7. un handoff de revue indépendante.

Ne pas modifier les états `features/.agents/a-identity/STATE.md` et
`features/.agents/orchestrator/STATE.md` dans ce lot. Ils restent en attente
jusqu'à acceptation humaine et revue indépendante.

## Revue obligatoire après correction

La correction ne vaut pas clôture.

Dans un contexte neuf, faire examiner le range exact
`<commit-de-départ>..<commit-candidat>` :

1. vérifier que la phrase normative est cohérente avec `spec/01`, la décision
   AID-001 et l'implémentation acceptée ;
2. vérifier que chaque nouveau test échoue réellement si l'immuabilité ou
   l'absence d'effet partiel est retirée, puis passe sur le candidat ;
3. vérifier que les tests n'utilisent pas un proxy, une assertion faible ou un
   chemin distinct de la production ;
4. relancer `review-gherkin-impacts` sur le delta de correction ;
5. ne passer les états à `COMPLETE` qu'après cette acceptation indépendante.

Si la correction reste strictement documentaire et test-only, ne pas rouvrir
automatiquement un audit complet des 53 features. Si elle touche finalement
du code de production, exiger une nouvelle revue sémantique ciblée avant la
revue d'impact globale.
