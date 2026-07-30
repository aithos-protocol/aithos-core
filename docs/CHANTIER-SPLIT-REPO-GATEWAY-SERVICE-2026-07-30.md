# Chantier — assainissement de la frontière protocole/service puis scission du dépôt

Date : 2026-07-30

Statut : **en cours — SPL-0, SPL-1, SPL-2 faits (2026-07-30).** Ce document est
le backlog canonique du chantier ; le brief d'amorçage est
[`PROMPT-REPRISE-SPLIT-REPO-GATEWAY-SERVICE-2026-07-30.md`](PROMPT-REPRISE-SPLIT-REPO-GATEWAY-SERVICE-2026-07-30.md).
Baseline figée : [`audits/split/baseline-2026-07-30.md`](audits/split/baseline-2026-07-30.md)
(638 tests, 1 365 scénarios cucumber, 6 495 steps, 36 `@wip`). Réserve
consignée : clippy était rouge sur non-macOS à l'état initial
(`aithos-cli/src/custody.rs:7`, code mort sous cfg) — corrigé en une ligne au
lot SPL-0.

Livrables attendus :

- `aithos-core` réduit au protocole, à ses cérémonies propriétaire et à sa CLI ;
- un dépôt `aithos-service` portant `aithos-gateway` + `aithos-provider` ;
- la grammaire de `validate_store_key` libérée de tout nom de consommateur ;
- deux CI indépendantes, un pipeline de release du bundle WASM, deux jeux de
  vecteurs.

---

## 1. Décision

Le dépôt `aithos-core` porte aujourd'hui six crates, dont deux (`aithos-gateway`,
`aithos-provider`) représentent à elles seules **88 764 lignes** Rust contre
**65 699** pour le protocole, sa CLI et sa façade WASM. La scission n'est pas une préférence
esthétique : **la frontière est déjà écrite dans le `LICENSE` racine**, qui
distingue deux régimes par chemin —

- *Aithos Core Additional Use Grant* : `rust/crates/aithos-core`,
  `rust/crates/aithos-bundle`, `rust/crates/aithos-cli`, `rust/crates/aithos-wasm`,
  `features/`, `docker/`, `.github/`, `ui-mockup/` ;
- *Aithos Service Additional Use Grant* : `rust/crates/aithos-provider` et
  `rust/crates/aithos-gateway`.

La cible retient donc **deux dépôts, pas trois** :

| Dépôt | Contenu | Licence |
|---|---|---|
| `aithos-core` | `aithos-core`, `aithos-bundle`, `aithos-cli`, `aithos-wasm`, `spec/`, `features/`, vecteurs protocolaires | BUSL — Core Grant |
| `aithos-service` | `aithos-gateway`, `aithos-provider`, leurs features, leurs vecteurs `p*` | BUSL — Service Grant |

**`aithos-cli` reste dans `aithos-core`.** Elle ne dépend que de
`aithos-core` + `aithos-bundle` + `aithos-wasm`, elle est le binaire de
conformance décrit par `spec/09-cli-and-conformance.md`, et `docker/Dockerfile`
la construit comme l'artefact du dépôt. L'extraire produirait un dépôt de
4 348 lignes en cassant la boucle spec ↔ CLI ↔ vecteurs, pour aucun gain.

**`aithos-provider` part avec la gateway**, pas dans un troisième dépôt : la
gateway le déclare en `dev-dependency` (les e2e P3 lancent le vrai service
in-process, acyclique), et les deux partagent le même régime de licence.

Le précédent technique existe et tourne : `aithos-client` est **déjà** un dépôt
séparé, consommé par `path` dep `../../aithos-client` et pinné par SHA dans les
deux workflows (`ref: c6f615123ca3dc83708ba029b898375409551719`). Le chantier
réapplique ce patron, il ne l'invente pas.

## 2. Constat vérifié le 2026-07-30

Toutes les références ci-dessous ont été relevées sur l'arbre de travail. Elles
sont le socle du plan ; **les revérifier avant d'attaquer chaque lot**, le code
étant arbitre en cas de divergence.

### 2.1 La règle de couche est bonne, et fuit en trois points

`rust/crates/aithos-gateway/src/lib.rs` énonce : *« only `core_bridge` (and its
`store_adapter` seam) imports from `aithos-core` / `aithos-bundle` »*. En code de
production, trois modules la violent :

| Fichier | `#[cfg(test)]` à | Usages de prod |
|---|---|---|
| `src/oauth.rs` | l. 2019 | 11 appels `aithos_core::wire::*`, `gamma::sha256_hex`, `gamma::ts_epoch` (l. 139 → 1840) |
| `src/ethos_catalog.rs` | l. 936 | l. 906, 907, 915 (`wire::multibase_to_ed25519_pub`, `wire::did_aithos`, `ids::Sid`) |
| `src/keyholder.rs` | l. 145 | l. 52 → 75 (`aithos_client::*`, `aithos_core::mandate::Mandate`) |

Ces usages sont des **utilitaires** (encodage multibase, SHA-256, parsing d'id),
jamais de la vérification de mandat ni d'append gamma : l'esprit de la règle
tient, la lettre non. À noter aussi que `aithos-client` — troisième crate
protocolaire, dépôt séparé — est importé par `keyholder.rs`, `ethos_backend.rs`
et `core_bridge.rs` alors que la règle ne le mentionne pas.

### 2.2 Le bloc `owner_*` de `core_bridge.rs` n'a AUCUN appelant runtime

Fait décisif, vérifié fonction par fonction : sur les 21 `pub fn owner_*` et les
6 lecteurs associés (`gamma_view`, `journal_notes_view`, `cert_grantee_pub`,
`cert_constraints`, `cert_perimeter`, `owner_read_journal_note`), **aucun n'est
appelé depuis `impl Bridge` (l. 404 → 2568) ni `impl Runner` (l. 2692 → 4732)**.
Les seuls appelants sont `src/main.rs` (la surface CLI du binaire) et les tests.

Seule exception dans la zone : `manifest_tool_pin` (l. 6170), appelée par
`src/hub.rs`, `src/connectors.rs` et `src/compiled_extensions.rs` — elle reste.

Corollaire : le bloc l. 4785 → 6470 (~1 685 lignes) est **extractible sans
toucher au runtime de la gateway**. C'est le lot le plus rentable du chantier, et
il est indépendant de toute décision de dépôt.

Sous-produit : `owner_read_briefing` (l. 5481) n'a **aucun appelant**, ni en
production ni en test. Code mort à supprimer.

### 2.3 `validate_store_key` nomme un consommateur — et le namespace d'accueil existe déjà

`rust/crates/aithos-bundle/src/lib.rs`, l. 156-157, la grammaire fermée des clés
de bundle énumère `"gateway/state.json"` et `"gateway/keys.json"` aux côtés de
`manifest.json`, `gamma/gamma.jsonl` et `e/self/root.enc`. Le protocole connaît
donc nommément un consommateur, alors que `spec/02-content-tree.md` **ne mentionne
la gateway nulle part** : ces deux clés sont une porte non spécifiée dans une
grammaire annoncée comme close.

Trois faits rendent la correction bien plus simple qu'attendu :

1. **Le namespace `x/` existe déjà** — `lib.rs` : `|| (segments[0] == "x" &&
   connector_object_accepted(&segments))`, avec `connector_object_accepted`
   (l. 115) qui accepte `x/<name>/…/<name>.{json,enc}`. Donc
   **`x/gateway/state.json` est déjà une clé valide aujourd'hui**, sans une
   ligne de grammaire nouvelle.
2. **Le verbe de mandat correspondant existe déjà** : `act.x.gateway.*` est le
   périmètre de la gateway (`core_bridge.rs:460`, et
   `ethos_catalog.rs:33-39` : `act.x.gateway.remote_read`,
   `connector_binding`, `connector_config`, `connector_effect`, `oauth_issue`,
   `refuse`). `spec/08-connectors.md:188` décrit `/x/<id>` comme un nœud protégé
   ordinaire. La migration aligne donc l'objet sur le nœud dont le verbe le
   gouverne déjà — ce n'est pas un contournement, c'est la place correcte.
3. **`gateway/keys.json` n'est écrit par personne.** Aucun producteur dans
   l'arbre ; à l'inverse, trois tests **asservissent son absence** :
   `tests/cli_surface.rs:84`, `tests/owner_surface.rs:191` et `:423`. La ligne de
   grammaire est de la surface morte, et un fichier de graines n'a de toute façon
   rien à faire dans un objet de bundle (custody locale du runner, comme
   `agent.id`).

Côté provider, aucune recopie de grammaire à corriger :
`rust/crates/aithos-provider/src/pathmap.rs:748` **compose**
`aithos_bundle::validate_store_key`, et son test `bundle_internal_keys_stay_outside_the_wire`
(l. 763-764) asserte que les deux clés `gateway/*` restent hors du wire —
conforme à `docs/REDLINE-A1-DRAFT2-PROPOSITION-GATE5-2026-07-20.md:134`, qui les
avait déjà classées « hors périmètre, ne PAS graver ».

Point de vigilance à lever au lot SPL-2 : `src/store_adapter.rs:321` lit
`gateway/state.json` **depuis le store répliqué**, et
`tests/e2e_delegated_ethos_remote.rs:213` le récupère par le remote. Il y a donc
une tension à trancher explicitement entre la redline (« pas une route wire ») et
l'usage réel (objet répliqué). La migration vers `x/gateway/state.json` la
résout dans le bon sens : sous `x/`, l'objet devient une route wire légitime,
couverte par `act.x.gateway.*`.

### 2.4 Couplages physiques cross-répertoire

**`include_str!` / chemins relatifs vers `vectors/`** — revérifié le
2026-07-30 au début du lot SPL-1 : le grep repo-entier renvoie en réalité
**~150 sites**, dont l'écrasante majorité dans les tests d'`aithos-core` et
d'`aithos-bundle`. Ces crates-là restent dans le même dépôt que `vectors/` :
leurs chemins ne cassent pas à la scission et sont hors périmètre du lot.
Les sites du périmètre sont ceux des crates qui partent (`aithos-gateway`)
ou dont le vecteur est consommé des deux côtés de la frontière
(`cb15` : CLI et WASM côté core, `aithos-client` côté dépôt séparé) —
**7 sites**, tous sous `#[cfg(test)]` :

```
aithos-gateway/src/core_bridge.rs:6875   ../../../../vectors/cb2-session-proof.json
aithos-gateway/src/core_bridge.rs:6881   ../../../../vectors/cb14-delegated-session-chain.json
aithos-gateway/src/public_tls.rs:1026    ../../../../vectors/p6-acme-txt.json
aithos-gateway/src/relay.rs:489          /../../../vectors/p3-tunnel-register.json
aithos-gateway/tests/cucumber.rs:9579    /../../../vectors/p3-tunnel-register.json
aithos-cli/tests/delegated_oauth.rs:72   ../../../../vectors/cb15-external-delegated-grant.json
aithos-wasm/src/lib.rs:389               ../../../../vectors/cb15-external-delegated-grant.json
```

(Le site `aithos-wasm` manquait au constat initial ; ajouté à la
revérification du 2026-07-30.)

`vectors/` (91 entrées, à plat) mélange vecteurs protocolaires (`a1-*`, `b2-*`,
`c1-*`, `cb*`) et vecteurs provider (`p1-*` → `p9-*`). La convention de nommage
marque déjà la frontière.

**Bundle WASM commité** — `aithos-gateway/assets/ceremony/` contient
`aithos_wasm_bg.wasm` (540 872 o) et `aithos_wasm.js` (24 268 o), tous deux datés
du 22/07 : un build figé de `aithos-wasm`, servi par la gateway pour la cérémonie
navigateur. Aujourd'hui la CI ne vérifie que `cargo check -p aithos-wasm --target
wasm32-unknown-unknown` — **la dérive entre l'artefact commité et le crate n'est
déjà pas détectée**. C'est le point qui demande le plus de plomberie neuve.

**Dépendances de workspace service-only** — sur le `[workspace.dependencies]` de
`rust/Cargo.toml`, sortent avec le service : `axum`, `tokio` (features
`rt-multi-thread`/`net`), `tower`, `reqwest`, `rustls`, `tokio-rustls`,
`rustls-pemfile`, `yamux`, `tokio-util`, `socket2`, `rcgen`, `instant-acme`,
`oauth2`, `openidconnect`, `tracing-subscriber`, `aws-config`, `aws-sdk-dynamodb`,
`aws-sdk-s3`, `aws-sdk-kms`, `aws-sdk-dynamodbstreams`, `aws-sdk-route53`,
`serde_yaml`, `chrono`. Gain immédiat : aujourd'hui **chaque changement de
protocole recompile et rejoue 29 144 lignes de tests gateway**.

### 2.5 Features : le corpus est propre

Trois corpus BDD physiquement séparés par couche, sans mélange structurel :

| Corpus | Emplacement | Harnais |
|---|---|---|
| Protocole | `features/` (racine, 18 fichiers) | `aithos-bundle/tests/cucumber.rs`, chargé l. 19725 via `CARGO_MANIFEST_DIR/../../../features` |
| Gateway | `aithos-gateway/tests/features/` (22) + `tests/enrollment_features/` (1) | `aithos-gateway/tests/cucumber.rs` + `support/g7b_steps.rs`, `support/oac0_steps.rs` |
| Provider | `aithos-provider/tests/features/{store,relay,tunnel,witness,remote}/` (11) | 5 harnais, un par lot |

Deux saignements seulement, tous deux mineurs :

1. `features/f-plus-constraints.feature:274` mentionne « the gateway » dans une
   phrase de prose, sans aucun step. Cosmétique.
2. `aithos-gateway/tests/features/gateway-delegated-client-surfaces.feature`
   (taggé `@wip @g4 @wasm @cli`) asserte sur la **surface WASM** et la **surface
   CLI** — *« it executes the same verify build and sign primitives as WASM »*.
   C'est un contrat de parité des clients du protocole, pas un comportement de
   gateway : il doit descendre dans `features/` racine, et ce déplacement est
   souhaitable **indépendamment** de la scission.

Les 22 autres features gateway sont strictement au niveau service (hub MCP,
OAuth AS, relay/TLS, bornes d'arguments, metering d'inférence, provisioning) :
aucune ne rejoue de vérification de chaîne ni d'invariant gamma.

## 3. Invariants non négociables

Aucun lot ne peut être déclaré fait s'il enfreint l'un de ces points :

1. **Aucun vecteur gelé n'est modifié.** Un vecteur peut être ajouté, jamais
   réécrit. Toute évolution de forme passe par un vecteur nouveau et une note de
   coexistence de versions.
2. **Aucune preuve gamma, aucun digest de manifest, aucun id de mandat ne
   change** du fait d'un déplacement de code. Un refactor qui déplace un octet
   de preuve est un bug, pas un refactor.
3. **La grammaire de `validate_store_key` reste close.** On généralise la forme,
   on n'ouvre pas la traversée. Toute clé nouvellement acceptée doit être
   justifiée par un test de rejet symétrique.
4. **Le protocole ne connaît aucun consommateur par son nom** à la sortie du lot
   SPL-2. C'est le critère de sortie du lot, pas un objectif moral.
5. **Fail-closed préservé.** Aucune ambiguïté de politique ne devient un accord
   du fait d'un déplacement de module.
6. **Les compteurs de tests ne baissent jamais.** Le harnais du lot SPL-0 les
   fige ; toute baisse est un blocage, y compris quand elle vient d'un `@wip`.
7. **Aucun `pub` nouveau sur `aithos-core` / `aithos-bundle` sans justification
   écrite** dans le lot correspondant. Le split ne doit pas devenir un prétexte
   à élargir la surface publique du protocole.

## 4. Découpage

Les lots sont ordonnés. **SPL-0 à SPL-5 se font dans le dépôt actuel, sans
toucher à Git** : à leur terme, 80 % du bénéfice architectural est acquis et la
scission devient un déplacement mécanique. Un arrêt du chantier après SPL-5
laisse le dépôt dans un état strictement meilleur qu'aujourd'hui — c'est
volontaire.

### SPL-0 — harnais anti-régression

**Objectif.** Rendre toute régression détectable avant la première modification.

**Actions.**

1. Créer un worktree dédié, ne jamais travailler sur la branche par défaut.
2. Capturer la baseline dans `docs/audits/split/baseline-2026-07-30.md` :
   sortie de `cargo test --workspace --manifest-path rust/Cargo.toml` avec le
   **nombre de tests passés par crate et par harnais cucumber**, la liste des
   scénarios `@wip`, et le SHA de tête.
3. Figer le SHA de `aithos-client` utilisé, et vérifier qu'il correspond au `ref:`
   des deux workflows.
4. Écrire un script `scripts/split-baseline.sh` qui rejoue la suite et **diffe
   les compteurs** contre la baseline, en échouant sur toute baisse.

**Critères de sortie.**

- `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`
  et `cargo test --workspace` verts, tous trois consignés avec leur durée ;
- `scripts/split-baseline.sh` vert sur un arbre non modifié ;
- baseline committée.

### SPL-1 — nettoyages préalables, sans déplacement de logique

**Objectif.** Retirer les cailloux avant de porter la charge.

**Actions.**

1. Supprimer `owner_read_briefing` (`core_bridge.rs:5481`), code mort confirmé.
2. Déplacer `gateway-delegated-client-surfaces.feature` vers `features/` racine
   et rebrancher ses steps sur le harnais de `aithos-bundle` (les scénarios sont
   `@wip` : le déplacement ne doit pas les activer, seulement les reloger).
3. Reformuler la mention « the gateway » de `features/f-plus-constraints.feature:274`
   en termes non nominatifs (« le point d'exécution », « le vérificateur en
   ligne »).
4. Neutraliser les 7 `include_str!` / chemins relatifs de la §2.4 : introduire un
   helper de résolution de fixture par crate (`tests/fixtures/vectors.rs`) qui lit
   depuis une variable d'environnement (`AITHOS_VECTORS_DIR`) avec repli sur le
   chemin actuel. Aucun vecteur n'est copié à ce stade — on ne fait que couper la
   dépendance à la profondeur de répertoire.

**Critères de sortie.**

- `scripts/split-baseline.sh` vert ;
- `grep -rn '\.\./\.\./\.\./vectors' rust/crates/aithos-gateway rust/crates/aithos-cli rust/crates/aithos-wasm`
  ne renvoie plus que le helper (gate rescopée le 2026-07-30 : la forme
  repo-entière du gate d'origine comptait aussi les ~143 sites
  core/bundle → vecteurs protocolaires du même dépôt, hors périmètre) ;
- aucun scénario `@wip` passé en actif.

### SPL-2 — libérer la grammaire du nom `gateway`

**Objectif.** `aithos-core` et `aithos-bundle` cessent de connaître un
consommateur par son nom.

**Décision arrêtée** (cf. §2.3) : généralisation vers `x/<consumer>/…`, pas
inscription de `gateway/` dans la spec.

**Gate de couverture wire — vérifiée le 2026-07-30, le lot procède.**
Constat sur `aithos-provider/src/pathmap.rs` :

- parse : `["x", id, rest @ ..]` accepte `x/gateway/state.json`
  (`validate_name("gateway")` ✓, `valid_file_segments(["state.json"])` ✓ —
  minuscules + point, pas de tête `.`) ;
- couverture GET : `ObjectPath::X(id, _) => act_connector(id)` ;
- couverture PUT : même ligne dans le bras d'écriture ;
- `act_connector` matche toute `PerimeterEntry::Act { connector == id }`,
  donc `act.x.gateway.*` (et tout `act.x.gateway.<action>` nommé) couvre
  GET/PUT `x/gateway/**`.

La tension §2.3 (redline « pas une route wire » vs objet répliqué) se
résout comme prévu : sous `x/`, l'état devient une route wire légitime
gouvernée par le verbe du même nœud ; `gateway/**` reste hors wire (le
scénario `@redline-a1` de `store-reads.feature:361` et le test
`bundle_internal_keys_stay_outside_the_wire` restent vrais tels quels).
Un test symétrique d'acceptation est ajouté au lot (critère de sortie 2).
Vérifié aussi : aucun vecteur gelé ne mentionne `gateway/state.json` ni
`gateway/keys.json` — le retrait de grammaire ne touche aucun vecteur.

**Actions.**

1. Supprimer l'arme `"gateway/keys.json"` de `validate_store_key`. Aucun
   producteur ; ajouter un test de rejet explicite et **conserver** les trois
   assertions d'absence existantes (`cli_surface.rs:84`,
   `owner_surface.rs:191`, `:423`).
2. Migrer `gateway/state.json` → `x/gateway/state.json`. La grammaire accepte
   déjà cette forme : la modification consiste à **retirer** l'arme nominative,
   pas à en ajouter une. Mettre à jour `core_bridge.rs:83` (`STATE_PATH`) et
   `store_adapter.rs:321`.
3. Écrire le chemin de migration : à l'ouverture d'un contexte, si
   `x/gateway/state.json` est absent et `gateway/state.json` présent, réécrire
   sous la nouvelle clé puis relire — **jamais** de suppression silencieuse de
   l'ancien objet dans ce lot.
4. Tester la couverture de mandat : un `act.x.gateway.*` couvre-t-il réellement
   la lecture/écriture de `x/gateway/state.json` sur le wire provider
   (`pathmap.rs`, lignes de couverture `act.x.<id>.*` → GET/PUT `x/<id>/**`) ?
   C'est le vrai gate du lot. S'il échoue, le lot s'arrête ici et la tension
   relevée en §2.3 remonte en décision.
5. Documenter dans `spec/02-content-tree.md` que la grammaire ne nomme aucun
   consommateur, et dans `spec/08-connectors.md` que `x/<id>` accueille l'état
   non secret d'un consommateur mandaté sur `act.x.<id>.*`.
6. Ajouter un vecteur **nouveau** couvrant la clé migrée (aucun vecteur gelé
   touché, invariant 1).

**Critères de sortie.**

- `grep -rn 'gateway/state.json\|gateway/keys.json' rust/crates/aithos-bundle rust/crates/aithos-core`
  ne renvoie plus rien ;
- le test provider `bundle_internal_keys_stay_outside_the_wire` est mis à jour et
  vert, et un test symétrique prouve que `x/gateway/state.json` est **accepté**
  sur le wire sous `act.x.gateway.*` ;
- `scripts/split-baseline.sh` vert ;
- migration prouvée par un test d'ouverture sur un store à l'ancienne clé.

**Rollback.** Le lot est un rename plus un pont de lecture : revert du commit,
l'ancienne clé n'ayant jamais été supprimée.

**Sortie du lot — 2026-07-30, gates atteintes.** Notes d'exécution :

- `bundle_internal_keys_stay_outside_the_wire` est resté vert **sans
  modification** : le wire n'a jamais accepté `gateway/*`, et ses deux
  entrées l'assertent toujours. Le test symétrique d'acceptation est
  `pathmap::migrated_gateway_state_rides_the_vault_subtree_row`.
- Custodie inchangée, décision consignée : la clé migrée reste routée
  sidecar en mode B (`sidecar_key`) et sautée par le sweep de réplication
  propriétaire — le lot retire une arme nominative sans créer de trafic
  wire nouveau ; la couverture wire est prouvée mais non exercée par le
  runtime.
- Découverte d'exécution : les deux stores canoniques valident la
  grammaire à l'accès (`checked_join` / `MemStore::get`) — le pont de
  migration lit donc l'ancien objet en accès brut de territoire pod
  (`legacy_state_bytes`), pas via `Store::get`. Attrapé par le harnais
  SPL-0 avant commit.
- Vecteur nouveau : `cb2-store-key-consumer-neutrality.json` (+ test
  bundle dédié). Aucun vecteur gelé touché.

### SPL-3 — isoler les helpers partagés de `core_bridge.rs`

**Objectif.** Préparer SPL-4 en séparant ce qui sert au runtime de ce qui sert
aux cérémonies. Aucun changement de comportement.

**Recomptage du 2026-07-30 (entrée de lot), après SPL-1/SPL-2.** La
suppression d'`owner_read_briefing` (SPL-1) a rendu cinq helpers du tableau
runtime-only (`zone_all_rows`, `ethos_row_is_covered`, `commitment_of`,
`public_read_current`, `view` : owner = 0) ; `read_state_migrating` (né en
SPL-2, 1/5), `cert_path` (9/15) et `bridge_err` (partout) manquaient au
tableau. Périmètre exécuté : **tout helper hors bloc owner** (partagé OU
runtime-only, y compris les utilisés par `hub`/`connectors`/`oauth`/
`proxy_mcp`/`main`) part dans `core_bridge/shared.rs` ; les six lecteurs
owner sans appelant runtime (`gamma_view`, `journal_notes_view`,
`cert_grantee_pub`, `cert_constraints`, `cert_perimeter`) et les helpers à
runtime = 0 restent avec le bloc owner pour SPL-4.

**Gate recalibrée le 2026-07-30 (arbitrage Mathieu, session split).** La
forme d'origine « descend sous 6 000 lignes » était inatteignable par
arithmétique : fichier à 7 036 lignes, masse déplaçable hors bloc owner
≈ 620 lignes (+ ~150 de tests), bloc owner (~1 500) réservé à SPL-4 et
`impl` intouchables — plancher ≈ 6 270. Gate remplacée par la forme
structurelle ci-dessous ; le < 6 000 arrivera mécaniquement au départ du
bloc owner en SPL-4.

**Actions.** Créer `core_bridge/shared.rs` et y déplacer les helpers utilisés
**des deux côtés** de la frontière (comptages d'origine du 2026-07-30,
runtime = l. 404-4732, owner = l. 4732-6870 — corrigés par le recomptage
ci-dessus) :

| Helper | runtime | owner |
|---|---|---|
| `read_json` | 10 | 18 |
| `hash_of` | 11 | 3 |
| `zone_all_rows` | 10 | 1 |
| `no_constraints` | 3 | 10 |
| `mint` | 3 | 6 |
| `ethos_row_is_covered` | 5 | 1 |
| `commitment_of`, `zone_rows`, `public_read_current`, `memory_rows`, `view`, `merge_server_pins`, `hub_manifest_paths` | 1-2 | 1-6 |

Les helpers à **runtime = 0** (`derived_owner`, `derived_succession`, `equip`,
`replace_hub_manifest`, `pin_hub_manifest`, `decode_pub`, `mint_entries`,
`preview_load`, `preview_status`, `effective_call_verdict`,
`describe_effective_policy`, `manifest_catalog_digest`) restent avec le bloc
owner : ils partiront en SPL-4.

**Critères de sortie.** `scripts/split-baseline.sh` vert ; `core_bridge.rs`
ne contient plus **aucune fonction libre hors bloc owner** (tout helper vit
dans `core_bridge/shared.rs` ; ne restent que les `impl`, les types, les
constantes, les `pub fn owner_*`, leurs six lecteurs et les helpers à
runtime = 0) ; aucun `pub` nouveau hors du crate (les re-exports gardent
les chemins publics existants).

### SPL-4 — remonter les cérémonies propriétaire

**Objectif.** Sortir du crate gateway la génération de mandats côté propriétaire.
C'est le cœur du chantier : sans ce lot, la scission exporterait la frappe de
mandats dans le dépôt service, sous une licence différente et hors de portée de
`features/e-mandates.feature`.

**Méthode.** Deux familles, arbitrées **par le compilateur**, pas par une liste
écrite d'avance :

- **Famille A — générique.** Tout `owner_*` qui ne touche ni
  `ApprovedManifest` / `ProposedManifest` / `ApprovedTool`, ni `crate::policy`,
  ni `crate::hub`, part dans `aithos-bundle` (ou un crate `aithos-owner` neuf,
  cf. décision ci-dessous), **génériquisé sur `S: Store`** au lieu de
  `GatewayStore` — 29 occurrences de `GatewayStore` dans le bloc, toutes des
  paramètres de store.
- **Famille B — liée au hub.** Ce qui dépend des manifests approuvés ou de la
  politique d'outils (`owner_enroll_server[s]`, `owner_reenroll_server`,
  `owner_read_hub_manifest`, `equip`, les digests de catalogue, les `preview_*`
  qui consomment un verdict d'appel) reste côté service : `ApprovedManifest` et
  la politique d'outils sont du domaine gateway, pas du protocole.

Procédure, une fonction à la fois : déplacer, génériquiser, compiler, rejouer la
baseline, committer. Jamais deux fonctions dans le même commit.

**Décision à prendre au début du lot** (documenter le choix dans ce fichier) :

- **(a) dans `aithos-bundle`**, aux côtés de `grants.rs` et de
  `tests/cb8_owner_grants.rs` où les grants propriétaire vivent déjà — plus
  simple, mais grossit un crate déjà à 39 930 lignes ;
- **(b) dans un crate `aithos-owner` neuf** dépendant de `core` + `bundle` —
  frontière plus nette, coût : un membre de workspace, une entrée de licence,
  une ligne de CI.

Recommandation : **(b)**, parce que les cérémonies propriétaire sont exactement
ce que la CLI et la gateway consomment toutes les deux, et qu'un crate dédié rend
cette consommation lisible au lieu de l'enfouir dans le bundle.

**Critères de sortie.**

- `grep -c 'pub fn owner_' rust/crates/aithos-gateway/src/core_bridge.rs` ne
  laisse que la famille B, et chaque survivant est justifié en une ligne dans ce
  document ;
- `impl Bridge` et `impl Runner` inchangés à la ligne près (diff vide sur
  l. 404-4732 hors renommages d'import) ;
- `scripts/split-baseline.sh` vert ;
- les tests qui appelaient les `owner_*` (9 fichiers de `tests/`, dont
  `cucumber.rs` et `support/g7b_steps.rs`) compilent contre la nouvelle
  localisation sans changement d'assertion.

### SPL-5 — unifier la surface CLI

**Objectif.** Une cérémonie propriétaire, un seul chemin.

**Constat.** `aithos-gateway/src/main.rs` (1 836 l.) expose 20 commandes dont **15
`Owner*`** (`OwnerInitJournal`, `OwnerInitContext`, `OwnerReplicateHistory`,
`OwnerGrantContext`, `OwnerGrantSessionDelegate`, `OwnerRevokeMandate`,
`OwnerDiscoverServer`, `OwnerProposeCompiled`, `OwnerEnrollServer`,
`OwnerGrantBriefing`, `OwnerGrantEthosRead`, `OwnerAddSection`, `OwnerSetBriefing`,
`OwnerPreviewMandate`, `OwnerConnectOauth`), qui doublonnent `aithos Grant` /
`Revoke` / `SectionAdd` / `LogShow`. En regard, `aithos-cli/src/main.rs` porte
**30 commandes** dans un `fn main()` unique de 743 lignes (l. 604 → 1347), sans
module par commande.

**Actions.**

1. Découper `aithos-cli` en `src/cmd/<commande>.rs`, un module par commande,
   `main()` réduit au dispatch. Aucun changement de surface : `tests/cli_surface.rs`
   (1 462 l.) est le filet.
2. Porter les commandes de la famille A vers `aithos`, sous un groupe `owner`.
3. Réduire le binaire `aithos-gateway` à `Keygen | Onboard | Run | AuditExport`
   plus les commandes de la famille B (enroll/preview), et sortir de `main.rs` le
   plumbing async (`serve_gateway`, `run_relay_plane`, `prepare_public_tls`,
   `renew_public_tls`) vers la lib, pour qu'il devienne testable hors process.
4. Prévoir une période de double surface : les commandes `Owner*` de
   `aithos-gateway` restent, marquées dépréciées, et délèguent au nouveau chemin.

**Critères de sortie.** Les deux `tests/cli_surface.rs` verts sans assertion
retirée ; `--help` des deux binaires consigné avant/après dans le lot ;
`scripts/split-baseline.sh` vert.

### SPL-6 — pipeline de release du bundle WASM

**Objectif.** Que l'artefact WASM servi par la gateway soit produit, versionné et
vérifié, au lieu d'être commité à la main.

**Actions.**

1. Ajouter à la CI `aithos-core` un job qui construit le bundle
   (`wasm-bindgen`), publie `aithos_wasm.js` + `aithos_wasm_bg.wasm` en artefact
   de release, et **publie leur digest**.
2. Ajouter au dépôt service une étape de fetch de l'artefact à version pinnée, et
   un test qui échoue si le digest local diverge du digest pinné.
3. Tant que les deux crates cohabitent, ajouter dès ce lot le test de digest —
   il ferme le trou de dérive **déjà ouvert** aujourd'hui (artefact du 22/07,
   jamais revérifié).

**Critères de sortie.** Le job de build WASM vert ; le test de digest rouge si
l'on modifie `aithos-wasm/src/lib.rs` sans régénérer l'artefact — à prouver par
une modification jetable.

### SPL-7 — scinder `vectors/`

**Objectif.** Chaque dépôt possède ses vecteurs, et les vecteurs partagés ont un
propriétaire unique.

**Actions.**

1. Classer les 91 entrées : protocolaires (`a*`, `b*`, `c1*`, `cb*`), provider
   (`p1` → `p9`), outillage (`bench-p4.py`, `deployed-replay-witness.py`,
   `README.md`).
2. Les vecteurs consommés **des deux côtés** (`cb2-session-proof.json`,
   `cb14-delegated-session-chain.json`, `cb15-external-delegated-grant.json`)
   restent dans `aithos-core`, sous licence CC-BY comme aujourd'hui, et sont
   exposés au dépôt service via l'artefact de release du lot SPL-6 ou un
   sous-module en lecture seule — **jamais dupliqués**.
3. `p1` → `p9` + `p6-acme-txt.json` + `p3-tunnel-register.json` partent avec le
   service.
4. Mettre à jour `vectors/README.md` pour énoncer la règle de propriété.

**Critères de sortie.** Aucune duplication d'octet entre les deux dépôts (à
prouver par comparaison de digests) ; les 7 sites du §2.4 résolvent via le helper
de SPL-1.

### SPL-8 — extraction du dépôt `aithos-service`

**Objectif.** Le déplacement Git, une fois qu'il ne reste plus que de la
mécanique.

**Actions.**

1. `git filter-repo` sur une copie, en conservant l'historique des chemins
   `rust/crates/aithos-gateway/**`, `rust/crates/aithos-provider/**`,
   `docker/{relay,store-api,witness}.Dockerfile`, les features et vecteurs du
   lot SPL-7, et les docs du lot SPL-9. 374 commits en tête de branche : vérifier
   que les commits touchant à la fois protocole et service sont conservés côté
   service en version tronquée, jamais perdus.
2. Créer le workspace du nouveau dépôt avec les dépendances de la §2.4, et
   `aithos-core` / `aithos-bundle` / `aithos-wasm` / `aithos-owner` en `git` dep
   pinnée par SHA — **copier exactement le patron `aithos-client`** déjà en place.
3. Reporter les `LICENSE` : `aithos-provider/LICENSE` (Service Grant) devient la
   licence du nouveau dépôt, et le `LICENSE` racine de `aithos-core` perd ses deux
   entrées service.
4. Scinder la CI : `ci.yml` de `aithos-core` perd les crates service ;
   `provider-image.yml` (déjà filtré sur `rust/crates/aithos-provider/**`) part
   tel quel ; ajouter au nouveau dépôt un `ci.yml` qui checkout `aithos-core` et
   `aithos-client` aux SHA pinnés.
5. Garder `aithos-provider` en `dev-dependency` de `aithos-gateway` — même
   workspace, aucun changement.

**Critères de sortie.**

- `cargo test --workspace` vert **dans les deux dépôts**, compteurs comparés à la
  baseline de SPL-0, somme des deux ≥ baseline ;
- `cargo build --release` vert dans les deux ;
- `docker build` vert pour `docker/Dockerfile` (côté core) et pour les trois
  Dockerfiles service ;
- les deux CI vertes sur un commit vide.

**Rollback.** Le dépôt d'origine n'est pas amputé avant que les deux CI soient
vertes. La suppression des crates dans `aithos-core` est le **dernier** commit du
lot, isolé et revertible.

### SPL-9 — documentation et index

**Actions.** Répartir les 43 documents de `docs/` : partent avec le service les
`DEMO-*`, `RUNBOOK-*`, `OLR-*`, `GATEWAY-*`, `HUB-MCP.md`, `INFRA-PROVIDER.md`,
`DEPLOYMENT-CONTAINMENT.md`, `EXPLORATION-DESKTOP-GATEWAY.md` ; restent `spec/`,
`DESIGN.md`, `MANIFESTO.md`, `CONFORMANCE.md`, `STANDARDS-COMPAT.md`, `CLI-*`,
les `audits/` protocolaires. Reconstruire les deux `docs/README.md` en respectant
la séparation norme / références / chantiers / archives déjà en place. Les liens
inter-dépôts deviennent des URL, jamais des chemins relatifs cassés.

**Critères de sortie.** Aucun lien mort (vérificateur de liens sur les deux
`docs/`) ; les deux index rendent compte de l'état réel, daté.

## 5. Gates de sortie du chantier

- somme des tests des deux dépôts ≥ baseline SPL-0, par harnais ;
- aucun vecteur gelé modifié (diff d'octets sur les fichiers antérieurs au
  chantier) ;
- `grep` de non-régression : aucun nom de consommateur dans
  `aithos-core` / `aithos-bundle` ;
- aucune duplication de vecteur ni d'artefact WASM entre les deux dépôts ;
- `cargo tree` prouvant que le graphe service résout bien vers les paquets
  `aithos-core` pinnés, et non vers deux copies ;
- les deux `LICENSE` cohérents avec les chemins réellement présents ;
- une démo de bout en bout rejouée après scission (le parcours de
  `DEMO-LEA-SCENARIO.md` ou son successeur), pour prouver que la frontière neuve
  ne casse pas l'intégration.

## 6. Charge indicative

| Lot | Charge |
|---|---|
| SPL-0 harnais | 0,5 j |
| SPL-1 nettoyages | 0,5 j |
| SPL-2 namespace | 1,5 j (dont 0,5 de décision sur la couverture wire) |
| SPL-3 helpers partagés | 1 j |
| SPL-4 cérémonies propriétaire | 3 j |
| SPL-5 surface CLI | 2 j |
| SPL-6 pipeline WASM | 1,5 j |
| SPL-7 vecteurs | 1 j |
| SPL-8 extraction Git + CI | 1,5 j |
| SPL-9 docs | 1 j |

**Total ~13,5 jours d'ingénierie**, soit 3 à 4 semaines calendaires avec revue.
Point d'arrêt utile : **SPL-0 → SPL-5, ~8,5 jours**, qui livre l'essentiel du
bénéfice architectural sans toucher à Git.

## 7. Hors périmètre explicite

- **Extraire `aithos-cli` dans son propre dépôt** — arbitré non, §1.
- **Un troisième dépôt pour `aithos-provider`** — arbitré non, §1.
- **Corriger les violations de couche de la §2.1** (`oauth.rs`,
  `ethos_catalog.rs`, `keyholder.rs`). Ce sont des utilitaires ; les router via
  `core_bridge` alourdirait le seam sans gain de sûreté. À traiter comme une dette
  documentée, et à reformuler dans le commentaire de `lib.rs` pour que la règle
  écrite corresponde à la règle appliquée — **y compris la mention manquante de
  `aithos-client`**.
- **Découper `core_bridge.rs` au-delà de ce que SPL-3 et SPL-4 exigent**, et
  découper `tests/cucumber.rs` (10 876 l.) : chantier distinct.
- **Toute évolution fonctionnelle.** Aucun comportement nouveau, aucun scénario
  `@wip` activé, aucune route publique ajoutée pendant ce chantier.
