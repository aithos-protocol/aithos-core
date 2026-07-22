# OLR OAuth — rollout production et rollback

Date : 2026-07-22  
Branche Gateway : `codex/olr-oauth-libs-upstream`  
Branche garde-fous Provider : `codex/provider-immutable-rollout`

## Décision d'architecture

La refactorisation OAuth est une évolution de `aithos-gateway`. Le Provider AWS
(Store, Relay, Witness) ne parse ni ne conserve les tokens OAuth amont. Il n'a
donc pas besoin d'être redéployé pour activer le moteur `oauth2`.

Le premier rollout sûr conserve le Provider actuellement déployé et change une
seule Gateway de démonstration. Une évolution Provider distincte ne doit être
mélangée à ce gate que si une incompatibilité de transport est démontrée.

## Artefact qualifié localement

- commit de stabilisation : `608b392`
- feature : `olr-oauth-libs`
- binaire release local macOS arm64 :
  `fd13e219201dffb1289d8ed481293a741a9c4f5c81fd6b85327b2d24bcc4ba0f`
- package Gateway complet : vert
- Cucumber : 296 scénarios, 1406 étapes, 0 échec
- `cargo fmt --check` et `cargo clippy -- -D warnings` : verts

Le binaire de production doit être reconstruit pour sa plateforme cible avec
`--locked` et sa propre somme SHA-256 doit être conservée avec le commit.

## Phase 0 — état de référence

Avant la bascule, conserver :

1. le binaire Gateway actuellement exécuté et son checksum ;
2. la configuration/profil actuellement actif ;
3. les ARN des task definitions ECS et digests ECR live du Provider ;
4. les résultats des probes de référence Store, Relay et Gateway ;
5. une copie hors Git des journaux/metrics de début de fenêtre.

Ne pas utiliser `origin/main` comme sauvegarde. Au 2026-07-22, cette branche ne
représente pas l'état Provider/Gateway actuellement démontré. Le rollback est un
retour vers un artefact, une configuration et, pour ECS, une task definition
précisément capturés.

La capture AWS nécessite une session fraîche :

```sh
aws sso login --profile aithos-prod
aws sts get-caller-identity --profile aithos-prod
```

Le runbook Provider détaillé se trouve dans
`infra/terraform/envs/prod/RUNBOOK-IMMUTABLE-ROLLOUT.md` de la branche
`codex/provider-immutable-rollout`.

## Phase 1 — canari Gateway uniquement

Construire la Gateway pour la cible de production :

```sh
cargo build --release --locked \
  -p aithos-gateway --bin aithos-gateway \
  --features olr-oauth-libs
shasum -a 256 target/release/aithos-gateway
```

Déployer un second processus ou une instance de démonstration, avec le même
Provider prod et un seul profil SaaS read-only :

```yaml
oauth:
  protocol_engine: oauth2
```

Ne pas activer l'override global tant que plusieurs profils partagent le même
processus. La bascule par profil réduit le blast radius.

## Phase 2 — gates live

Ordre recommandé :

1. Notion read-only : discovery/consent, `tools/list`, lecture d'une page,
   refresh et reconnexion Claude Cowork ;
2. observation de `GET /control/v1/status`, section `upstream_oauth`, sans token
   ni secret dans les logs ;
3. Gmail read-only : discovery/consent, liste/recherche bornée, refresh et
   reconnexion Cowork ;
4. expiration/revocation volontaire : échec fail-closed et demande de nouvelle
   authentification ;
5. maintien du tunnel Relay et des écritures Store attendues par la démo.

Critères de GO : aucun fallback implicite, aucune fuite de secret, callback
one-shot, refresh atomique et comportement fonctionnel identique au moteur
native. Étendre ensuite profil par profil pendant la fenêtre d'observation.

## Rollback Gateway

Le Vault n'est pas migré par la refactorisation. Le rollback normal est donc :

1. remettre `protocol_engine: native` sur le profil concerné ;
2. redémarrer uniquement la Gateway canari ;
3. rejouer health, discovery et un appel read-only ;
4. si le problème précède la configuration, restaurer le binaire et le checksum
   capturés en phase 0.

Laisser `olr-oauth-libs` compilé dans le binaire n'active pas le moteur : la
configuration reste le coupe-circuit opérationnel.

## Si une évolution Provider devient nécessaire

La faire dans une fenêtre séparée, un service à la fois, avec une task
definition pinnée sur le digest ECR `sha256` et non `prod`. La branche
`codex/provider-immutable-rollout` sépare les tags et digests Store, Relay et
Witness pour empêcher qu'un seul changement redéploie les trois services.

Le rollback ECS réaffecte au service l'ARN de task definition capturé avant
l'apply, puis attend `services-stable`. Si l'ancienne task utilisait le tag
mutable `prod`, son manifeste doit d'abord être conservé sous un tag de rollback
pointant le même digest.

## STOP conditions

- identité AWS non vérifiée ou session SSO expirée ;
- task definition/digest live non capturé ;
- plan Terraform contenant une destruction ou un autre service ;
- image cible disponible uniquement sous `prod` ;
- erreur de refresh, rejeu callback, divergence de Vault ou secret visible ;
- probe Store/Relay dégradé avant même la bascule.

Dans ces cas, ne pas déployer et conserver la prod telle quelle.
