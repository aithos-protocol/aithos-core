# OLR-0 — Corpus de régression OAuth libs (amont)

Date : 2026-07-22
Branche : `feature/olr-oauth-libs-upstream`
Base locale seed : `044c497`
Suite BDD ancre : `gateway-upstream-oauth.feature`

Ce corpus doit rester **vert** avant et après chaque bascule de moteur
(OLR-1 seam, OLR-2 `oauth2`, OLR-3 OIDC). Aucun token, secret, code ou
verifier ne doit apparaître dans les artefacts publics listés.

## 1. Vecteurs positifs (compatibilité)

| ID | Intention | Ancre / observation attendue |
| --- | --- | --- |
| P-01 | Config secretless Vault-only + callback public | parse config OK ; pas de bearer concurrent |
| P-02 | Consent owner : PKCE S256 + state imprévisible | URL authorize contient `code_challenge_method=S256` et `state` |
| P-03 | Callback échange code → token set Vault | état `connected` ; HTML callback sans secret |
| P-04 | Access token injecté uniquement sur l'upstream protégé | bearer présent sur le fil cible, absent ailleurs |
| P-05 | Access expiré → un refresh puis retry | une seule rotation ; appel upstream authentifié |
| P-06 | Client public : jamais de `client_secret` résolu/envoyé | auth method `none` |
| P-07 | Discovery protected-resource → metadata AS validée | resource/issuer pinnés |
| P-08 | MCP scope-less : DCR d'un client public PKCE | registration durable isolée Vault |
| P-09 | Profil Google offline : paramètres typés approuvés seulement | `access_type` / `prompt` selon intent ; pas d'extras hostiles |
| P-10 | Refresh sans nouveau refresh_token : conserve l'actuel | pas d'effacement du refresh existant |

## 2. Vecteurs négatifs (fail-closed)

| ID | Intention | Verdict attendu |
| --- | --- | --- |
| N-01 | OAuth + bearer credential sur le même serveur | rejet config |
| N-02 | Refresh `invalid_grant` / échec | **aucun** appel upstream non authentifié |
| N-03 | Callback avec `state` inconnu ou rejoué | refus ; pas d'écriture token |
| N-04 | `redirect_uri` différente de la config | refus |
| N-05 | Metadata discovery hors origine / issuer movable | refus |
| N-06 | Réponse token malformée / `token_type` non bearer / `expires_in=0` | unavailable ; pas de custody partielle |
| N-07 | Corps metadata / token / registration > borne | refus borné avant parse large |
| N-08 | Redirect HTTP suivi par le client OAuth | politique `none` — pas de follow |
| N-09 | Endpoint hors loopback en clair (non-TLS) | rejet config / runtime |
| N-10 | Erreur publique ou log contenant code/token/secret/verifier | redaction obligatoire |

## 3. Concurrence et atomicité

| ID | Intention | Preuve |
| --- | --- | --- |
| C-01 | Deux instances gateway, callbacks parallèles même `state` | CAS Vault : un seul succès ; second = rejeu avant token endpoint |
| C-02 | Refresh concurrent sur access expiré | une rotation gagnante ; Vault cohérent |
| C-03 | Échec d'écriture Vault après token endpoint | ancien état conservé (pas de half-apply) |

## 4. Matrice fournisseur (à remplir aux live gates)

| Famille | Profil démo | Live gate OLR-2+ | Notes |
| --- | --- | --- | --- |
| AS de test local | harness cucumber | local | ancre permanente |
| Google offline read-only | connector profile | TBD | OLR-2 premier profil réel |
| OIDC explicite | profil `oidc:` | TBD OLR-3 | sinon ne pas activer `openidconnect` |

## 5. Commandes de rejeu (indicatif)

```bash
cd rust
cargo test -p aithos-gateway --test cucumber -- \
  --input tests/features/gateway-upstream-oauth.feature
cargo check -p aithos-gateway
cargo check -p aithos-gateway --features olr-oauth-libs
```

Les tests de parité OLR-1 (ancien vs nouveau moteur sur les mêmes vecteurs)
s'ajoutent **sans** retirer ce corpus.

## 6. Non-régression hors amont

Ne pas exiger le vert de `gateway-oauth.feature` (AS entrant) pour valider
OLR-1/2/3. Le rejouer seulement si un changement accidentel touche `oauth.rs`
/ `oauth_state.rs`.
