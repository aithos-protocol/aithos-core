# ADR — OLR : OAuth amont sur bibliothèques standard

Date : 2026-07-22
Statut : **ACCEPTÉE pour OLR-0** (spike deps + cartographie + corpus).
Périmètre de cette branche : **client OAuth amont gateway uniquement**.
Hors périmètre : provider / relai `aithos.fr`, OAuth entrant G3/G4 (OLR-6).

Référence chantier :
`docs/CHANTIER-REFACTOR-OAUTH-LIBRAIRIES-STANDARD-2026-07-22.md`.

## 1. Décision

Adopter une architecture hybride pour le **client OAuth amont** de
`aithos-gateway` :

- `oauth2` **5.0.0** porte Authorization Code, PKCE S256, échange de code,
  refresh et erreurs protocolaires ;
- `openidconnect` **4.0.1** porte la validation OIDC (ID Token, JWKS, issuer,
  nonce, claims) **uniquement** lorsque le profil connecteur déclare OIDC ;
- Aithos conserve l'autorité métier : profils, policies, Vault, state durable
  one-shot, redaction, Gamma, cérémonie G4, contrôles d'origine et bornes HTTP.

Une bibliothèque produit un résultat protocolaire. L'adaptateur Aithos le
revalide et le lie au connecteur, au compte, au tenant et au state attendus.
Elle n'est jamais source d'autorité Aithos.

## 2. Cartographie des chemins (état actuel)

### 2.1 Amont (cible OLR-1 → OLR-5)

| Module | Rôle |
| --- | --- |
| `upstream_oauth.rs` | Consent PKCE, callback public, token/refresh, injection bearer, registry |
| `oauth_discovery.rs` | RFC 9728 / 8414 bornés, no-redirect, pin resource/issuer |
| `oauth_registration.rs` | Static / DCR RFC 7591, isolation Vault des credentials |
| `config.rs` (`UpstreamOAuthConfig`) | Profils, auth methods, endpoints, Vault refs |
| `proxy_mcp.rs` / `connectors.rs` | Consommation du token amont sur le fil protégé |

Contrat BDD de référence : `tests/features/gateway-upstream-oauth.feature`.

### 2.2 Entrant (hors branche, OLR-6 plus tard)

| Module | Rôle |
| --- | --- |
| `oauth.rs` | AS `gateway_as` (G3) + cérémonie déléguée (G4) |
| `oauth_state.rs` | State AS durable (mémoire / Vault) |

Contrat BDD : `gateway-oauth.feature`, `gateway-oauth-durable.feature`.
Les crates `oauth2` / `openidconnect` sont **clientes** : elles ne remplacent
pas un Authorization Server.

### 2.3 Frontière DCR / CIMD (décision OLR-0)

| Pièce | Décision |
| --- | --- |
| Construction URL authorize + PKCE + token + refresh | migrer vers `oauth2` (OLR-2) |
| Parsing erreurs token OAuth | migrer vers `oauth2` |
| Validation OIDC ID Token / JWKS | `openidconnect` si profil OIDC (OLR-3) |
| Discovery RFC 8414/9728 | **rester adaptateur Aithos** ; réutiliser des types lib seulement s'ils n'affaiblissent pas les pins d'origine |
| DCR RFC 7591 / CIMD | **rester Aithos** jusqu'à preuve de couverture équivalente (OLR-4) |
| State pending, one-shot callback, Vault token set | **rester Aithos** |
| Auth methods `none` / `client_secret_post` / `client_secret_basic` | conservées ; le client HTTP et la redaction restent Aithos |

## 3. Dépendances figées (spike OLR-0)

| Crate | Version | Licence | MSRV crate | Features retenues |
| --- | --- | --- | --- | --- |
| `oauth2` | `5.0.0` | MIT OR Apache-2.0 | 1.65 | `default-features = false`, `reqwest`, `rustls-tls` |
| `openidconnect` | `4.0.1` | MIT | 1.65 | `default-features = false`, `reqwest`, `rustls-tls` |

Contraintes workspace observées :

- toolchain locale : Rust **1.95** / edition **2021** — compatible ;
- TLS : **rustls uniquement** (aligné `reqwest` workspace) — **pas** de
  `native-tls` ;
- `openidconnect 4.0.1` tire `oauth2 5.0.0` — graphe cohérent ;
- feature Cargo gateway **`olr-oauth-libs`** (non default) : active les deps
  sans changer le binaire par défaut.

Politique de mise à jour :

1. pas de bump mineur/majeur sans rejouer le corpus OLR-0 et la suite
   `gateway-upstream-oauth` ;
2. interdit d'activer `native-tls`, `pkce-plain`, ou features « accept-* »
   d'`openidconnect` sans ADR dédiée ;
3. rollback = désactiver la feature / le profil bascule, sans migration Vault.

## 4. Menaces et contrôles conservés

| Menace | Contrôle Aithos (non négociable) |
| --- | --- |
| Rejeu callback / state | state durable consommé par CAS avant effet token ; broker sans CAS refusé |
| Open redirect | redirect URI exacte de config, jamais dérivée du callback |
| SSRF / issuer movable | discovery bornée, no-redirect, pins resource/issuer |
| Fuite secrets | zeroize + redaction logs/erreurs publiques |
| Token hors custody | Vault only pour l'état durable |
| Refresh partiel | rotation atomique ; conserver l'ancien état si l'écriture échoue |
| Confusion identité ≠ autorité | OIDC validé ≠ mandat / Gamma Aithos |

## 5. Conséquences

- OLR-1 introduit une seam interne derrière l'implémentation actuelle, sans
  changer routes publiques ni schéma Vault.
- OLR-2 bascule d'abord un profil démo read-only via `oauth2`.
- OLR-6 (entrant) n'ouvre qu'après stabilisation amont.
- Aucun impact attendu sur le provider / relai `aithos.fr`.
- Le broker Vault KV v2 de référence porte le CAS inter-instance ; tout broker
  OAuth alternatif doit implémenter `CredentialBroker::compare_and_store`.

## 6. Sortie OLR-0

- [x] Cartographie amont / entrant
- [x] Décision DCR/CIMD / discovery restent Aithos pour l'instant
- [x] Versions, licences, features et politique de bump figées
- [x] Corpus de régression écrit
  (`docs/OLR-0-REGRESSION-CORPUS-OAUTH-LIBS-2026-07-22.md`)
- [x] Deps optionnelles déclarées sous feature `olr-oauth-libs`
