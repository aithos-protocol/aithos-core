# Aithos Gateway — Handoff (reprise en contexte neuf)

**But.** Reprendre le chantier du gateway (runner conteneurisé) sans rien reperdre.
Complète `GATEWAY-BOOTSTRAP.md` (le pourquoi/quoi) avec l'état exact du code et
les leçons d'environnement. Session initiale : 2026-07-10.

> **ÉTAT EXPRESS (2026-07-16 nuit, 12ᵉ session gw — G2 + G6 CLOS, gates réels passés).**
> Profil VM hybride confirmé (egress 000, unlink DENIED sur montage, pas de
> toolchain VM), protocole cloud+janitor §5 à la lettre, HEAD d'entrée
> `6fdfe3c`, baseline revalidée À L'IDENTIQUE avant tout travail. **Contrats
> d'abord** (`3b451ae`, seuls) : `gateway-streamable.feature` (13 scénarios) +
> `gateway-ethos-read.feature` (17), sonde de parse détag/re-tag des deux
> côtés. Décisions Mathieu consignées (AskUserQuestion, 2026-07-16) : session
> id STATELESS (émis à l'initialize depuis l'EntropySource injecté, écho,
> jamais exigé — l'autorité reste à la chaîne, G5 apportera les chaînes de
> session), GET/DELETE → 405, batch refusé -32600 (aligné 2025-06-18), Origin
> validé fail-closed MAINTENANT (MUST anti DNS-rebinding) ; G6 = surface
> DÉRIVÉE des mandats, jamais un toggle — recalculée par appel (couvert ∩
> lignes ∩ contenu), public = frontière de lisibilité servie SANS grant à
> toute session connectée, self jamais par défaut (le grant explicite est
> gravé au contrat mais reste @wip : la résolution self déléguée est un lot
> core séparé, vectors-first), `ethos.context` = briefing + corps public +
> index scellé sans corps. **G2 clos** (`d17d77b`) : coquille transport axum —
> notification → 202 corps vide (le bug -32601 sur `notifications/initialized`
> est mort), id-less non-notification → 400 fail-closed (jamais d'acte
> silencieux), `ping` → `result:{}`, `Mcp-Session-Id` opaque émis/écho, 405
> GET/DELETE, batch → -32600, Origin non-local → 403 avant tout JSON-RPC ;
> gate RÉEL : MCP Inspector ET Claude Code se connectent, listent, appellent —
> zéro erreur de protocole des deux côtés, audit-export montre les 2 actes.
> **G6 clos** (`1350e20`) : outils natifs `ethos.read/list/context` servis par
> le hub (jamais relayés), surface dérivée du SCAN des certificats — toute
> chaîne valide vers la clé agent, quel que soit le geste émetteur (owner CLI,
> sous-mandat de délégué, G8.c demain) — grant à chaud allume, révocation à
> chaud éteint (refus nommant le mandat révoqué), chaque corps circle ouvert =
> une entrée `ethos.read` sous la chaîne qui a lu (lecture injournalisable =
> appel refusé, précédent C2), étagères `briefing/` EXCLUES des outils de
> données (leur surface dédiée reste briefing.read — chemin chaud démo Léa
> byte-identique), préfixe `ethos` réservé partout (RESERVED_PREFIXES 2→3,
> is_reserved_server, hub validate_server), gestes `owner-grant-ethos-read`
> (self refusé pédagogiquement, ligne circle à l'agent ET à l'auditeur — le
> précédent K) et `owner-add-section` (GAPS beat 2), surfaces harnais
> `owner_revoke_mandate_id` / `owner_issue_ethos_read_subchain` (pré-M3/G8.c) ;
> gate RÉEL : zones remplies par CLI, session sans `read.circle` AVEUGLE
> (liste sans ligne circle, refus nommant le périmètre), grant à chaud puis
> Claude Code lit la mémoire scellée et s'en sert (« 550 000 € »), lectures au
> gamma, zéro contenu dans les logs. Suite : gateway **63 unit / 4 CLI /
> 119 scénarios-627 steps / 6 e2e (e2e_demo_lea compris) / 7 owner /
> 5 équivalence** ; core+bundle+cli inchangés (**100** + **229/906**) ; clippy
> `-D warnings` + fmt clean. Restent `@wip` : 1 ethos-read (self-serves, lot
> core), 14 gateway-mandates (M3/M4/M6), 8 e-mandate-sections (M4). Prochain :
> G7 (surface de preuve) / G8.a-c-d (sessions dédiées) / G3 (OAuth, chemin
> critique) → `docs/HANDOFF-GATEWAY-G2-G6-DONE-2026-07-16.md`.

> **ÉTAT EXPRESS (2026-07-16, 10ᵉ session gw — surface mandats M0+M1+M2)** :
> **GATE M0 TRANCHÉ** (AskUserQuestion, les six recos confirmées par
> Mathieu) : (a) mandats restreints = **roots owner multiples**,
> containment vérifié à l'ÉMISSION contre politique Ethos ∩ manifeste ;
> (b) **un seul mandat actif par (Ethos, keypair)** ; (c) clés de
> contraintes inconnues en sous-délégation = **refus fail-closed** ;
> (d) N mandats émis mais **UN runner actif par contexte** jusqu'au
> RemoteStore (limite documentée) ; (e) chantier APRÈS le gate
> répétition démo Léa ; (f) nommage `owner-issue-mandate` /
> `owner-revoke-mandate` / `owner-preview-mandate`. **Lot M1 clos**
> (`aa02353`, contrats SEULS) : `tests/features/gateway-mandates.feature`
> (16 scénarios @wip — émission restreinte ⊆ manifeste, bornes héritées
> jamais éditées, cardinalité (b), deux délégués/attribution/révocation
> ciblée, invariant « jamais de ligne vault pour un act.* »,
> preview=décision, read-model), `features/e-mandate-sections.feature`
> (8 scénarios id=, reco M4 : id= couvert par id= identique ou zone
> entière, jamais par dir=), matrice d'atténuation PAR FAMILLE ajoutée à
> `features/f-plus-constraints.feature` (26 scénarios @wip, décision (c)
> gravée) ; sondes de parse validées des DEUX côtés (89ᵉ/204ᵉ scénario
> apparaît au détag). **Lot M2 clos** (`f8cbc88`) : moteur de politique
> effective **PUR** dans `core_bridge` (`owner_preview_mandate` /
> `owner_preview_call`, chargeur owner-side état+cert+DID+révocations+
> manifestes scellés, JSON versionné `aithos-effective-policy-v1` :
> statut actif/expiré/révoqué/not_yet_valid/invalide + outils
> granted/covered/served + bornes héritées + contraintes), CLI
> `owner-preview-mandate` (`--server`×N, `--call`/`--args` dry-run,
> `--at` T injecté), **5 tests d'équivalence** `tests/policy_equivalence.rs`
> rejouant la matrice grants/bounds (grants/denies/défauts de classe,
> toutes les familles de bornes, inconnu, fenêtre expirée) avec égalité
> **LITTÉRALE** (code + message) contre `Runner::authorize` +
> `check_bounds`, 2 scénarios preview détaggés, 2 tests owner_surface
> neufs. **Le chemin chaud n'est PAS rebranché** (l'équivalence est la
> preuve ; rebranchement = lot ultérieur). Suite : **90 scénarios /
> 481 steps**, 62 unit, 4 CLI, **6 e2e**, **7 owner**, **5 équivalence** ;
> core+bundle+cli inchangés (97 + 203/826) ; clippy `-D warnings` et fmt
> clean. Restent `@wip` : 14 gateway-mandates (M3/M4/M6), 8 sections
> (M4), 26 atténuation (M5) → `docs/HANDOFF-MANDATES-M3-2026-07-16.md`.

> **ÉTAT EXPRESS (2026-07-15 nuit, 9ᵉ session gw — démo Léa K+D)** :
> **RÉPÉTITION GÉNÉRALE VERTE, zéro `@wip` sur les quatre contrats
> démo.** **Lot K clos** (`b2f5b69`) : `briefing.read` natif servi des
> zones public+circle des contextes grantés (`self` jamais — hors de
> portée structurellement), surface conditionnelle (descripteur
> `tools/list` + `initialize.instructions`, recalculés PAR APPEL — une
> édition owner bascule la surface sans redémarrage), chaque lecture
> circle journalisée `ethos.read` sous un **pen briefing dédié**
> (`owner-grant-briefing`, geste séparé, survit au re-enrollment ;
> ligne circle livrée à l'agent ET à l'auditeur), `owner-set-briefing`
> création + rewrite circle (public/self write-once v1, public = clair
> keyless sans entrée de lecture — frontière de lisibilité,
> documenté), préfixe `briefing` réservé dans toute tool map et tout
> nom de serveur. **Lot D clos** (`0db670e`) : les 8 beats détaggés
> (monde Innoestate wire : faux Vault + 3 MCP sockets, vrais brokers)
> + e2e réseau `tests/e2e_demo_lea.rs` (vrai binaire, édition de
> consigne À CHAUD par le CLI owner pendant que le gateway tourne,
> balayage sentinelles, note self jamais en clair sur disque) ;
> `owner-enroll-server` accepte N `--proposal` → UN mandat agent
> couvrant l'union des grants (`owner_enroll_servers`, all-or-nothing,
> approvals ventilées par outil, collisions refusées, `--replace`
> mono-serveur) ; **auditeur de contexte élargi**
> `kind=action`+`kind=ethos.read` (les lectures de briefing font
> partie du replay ; toute requête plus large reste refusée — mono
> `onboard` inchangé) ; refus `bound_violated` porteurs de leur
> **détail pédagogique en clair** (payload.detail = champ, valeurs
> fautives, règle approuvée — la politique scellée de l'owner, les
> autres refus gardent le code nu). Runbook `docs/DEMO-LEA.md` (état
> connecteurs vérifié 2026-07-15 : Notion self-hosted HTTP+bearer
> statique prêt coffre ; MCP officiels Google Gmail/Calendar distants
> HTTP+OAuth en Developer Preview — access tokens courts au coffre ;
> caveats sessions stateless/TLS `rustls-tls`/bornes racine). Suite :
> **88 scénarios / 473 steps**, 62 unit, 4 CLI, **6 e2e**, 5 owner ;
> clippy `-D warnings` et fmt clean. **GATE : répétition générale avec
> Mathieu en conditions réelles avant le jour J** →
> `docs/HANDOFF-DEMO-LEA-DONE-2026-07-15.md`.

> **ÉTAT EXPRESS (2026-07-13, 6ᵉ session gw — H1/H2/H3)** : **HUB MCP
> GOUVERNÉ RUNTIME VERT.** Décisions complémentaires validées par Mathieu :
> manifest `/x/<server>` par Ethos, contrôle drift local chaque call + amont à
> l'ouverture/sur erreur, classes `read|write`, bearer config v1, collisions
> post-aplatissement sans bannir `__`, `HUB-MCP.md` suivi. H1 config v3 fermé
> dans `6b580ff`; H2 discover/approval/pin/grants owner fermé dans `f915d34`;
> H3 implémenté et QA verte (`4fc4b4d`) : pins ouverts
> via la ligne gateway, `tools/list` hors-ligne exact et limité aux reads,
> serveur partagé entre deux Ethos, nom brut restauré, `act.x.<server>` + xref,
> write connu caché/refusé, drift `manifest_drift` gouverné, surface hors
> `tools/*` fermée, bearer amont. Suite : **41 scénarios / 202 steps**, 36 unit,
> 4 CLI, 5 owner surface, **4 e2e réseau** ; clippy `-D warnings` et fmt clean.
> **Zéro `@wip` hub** : H2b re-enrollment est désormais clos (`--replace`, même
> clé agent, nouveau pin/mandats, révocation politique des anciens). **H4 est
> clos** : vrai binaire, deux MCP
> HTTP dont un partagé, bearer wire-only, audit par contexte et restart bloqué sur
> drift.

> **ÉTAT EXPRESS (2026-07-15 soir, 8e session gw — démo Léa W+P)** : le
> scénario de référence `docs/DEMO-LEA-SCENARIO.md` est VALIDÉ et les
> quatre contrats de la démo committés seuls (`190d6b4`). **Lot W clos**
> (`0e59e91`) : décision d'octroi ≠ classe de risque — writes grantables
> explicitement, défauts sûrs, révocation politique au re-enrollment.
> **Lot P clos** (`56d2a14`) : bornes d'arguments owner-approuvées
> (`one_of`/`time_slots`/`forbid`/`require`/`max_items`) scellées au
> manifeste HORS pin hash, check post-authorize pré-log, refus
> `bound_violated` pédagogique, zéro hit coffre/amont, CLI `--bound`.
> Suite : **72 scénarios / 355 steps**, 61 unit, 4 CLI, 5 e2e, 5 owner ;
> Cucumber désormais SÉQUENTIEL (starvation tokio sous mondes à sockets —
> ne pas retirer). Restent `@wip` : `gateway-briefing` (lot K) et
> `gateway-demo-lea` (lot D) → `docs/HANDOFF-DEMO-LEA-K-D-2026-07-15.md`.

> **ÉTAT EXPRESS (2026-07-15, 7ᵉ session gw — coffre Vault)** : **CREDENTIALS
> MCP BROKERÉS VERTS (V0→V3).** Les tokens MCP sortent du YAML : références
> non secrètes `credential_brokers`/`servers[].credential`, adapter
> **HashiCorp Vault KV v2** (`src/credentials.rs`), résolution PAR APPEL dans
> `HttpUpstream::forward()` après authorize + log-before-relay, refus
> `credential_unavailable` fail-closed sans contact amont, rotation KV sans
> config, erreurs expurgées structurellement, `SecretValue` sans
> Debug/Serialize/Clone zeroizé au drop. Commits `9dd81fc` (contrat @wip)
> → `ea224d3` (config+abstraction) → `34dfd22` (adapter+câblage+détag) →
> `916ecb3` (e2e réseau + runbook `DEMO-GATEWAY-VAULT.md` + exemple
> `demo_mcp`). Suite : **53 scénarios / 269 steps**, 55 unit, 4 CLI,
> **5 e2e réseau** (dont `e2e_vault`), 5 owner ; clippy `-D warnings` et fmt
> crate clean. Couture `bearer_token` conservée LEGACY (exclusivité
> enforced) ; V4 LLM, TLS reqwest, AppRole, writes grantés et `resources/*`
> restent ouverts → `docs/HANDOFF-GATEWAY-VAULT-DONE-2026-07-15.md`.

**Branche active imposée : `feat/obligations`** (ne jamais switcher). Crate :
`rust/crates/aithos-gateway/`.

---

## 1. État : MVP audit VERT (première brique vendable)

`tests/features/gateway-audit.feature` — **5 scénarios / 29 steps verts**, plus
8 tests unitaires (config, policy), 3 tests de surface CLI (`cli_surface.rs`,
binaire réel) et **1 e2e réseau** (`e2e_http.rs` : binaire `run` en process
enfant, faux MCP amont sur vraie socket, JSON-RPC sur le fil, audit-export —
le parcours client complet en local). Le parcours vendu fonctionne :

on plugge un agent (config YAML + onboard), les lectures passent et sont
tracées, les écritures et l'inconnu sont refusés fail-closed et les REFUS sont
tracés, le kind est imposé par l'opération (jamais par l'appelant), la chaîne
se vérifie offline, et un auditeur tiers exporte exactement sa tranche
(`read.gamma#kind=action`) — requête plus large refusée par le certificat.

```
cargo test  -p aithos-gateway --manifest-path rust/Cargo.toml   # tout le crate
cargo clippy -p aithos-gateway --all-targets -- -D warnings      # clean
```

## 2. Décisions prises (avec Mathieu, 2026-07-10)

- **Transport MCP v1 : Streamable HTTP** (JSON-RPC sur POST `/mcp`, stateless).
  stdio amont = plus tard, via wrapper, sans toucher le flux.
- **Ethos v1 : disque local** (FsStore), le cloud DOIT rester possible →
  `StoreConfig` parse déjà `s3` mais `store_adapter` le refuse (fail-closed).
- **Config entreprise : YAML whitelist** `tools: {outil: read|write}`,
  `deny_unknown_fields`, default-deny pour tout outil absent.
- **proxy_llm v1 (post-MVP) : OpenAI-compatible** d'abord (stub en place).

## 3. Architecture posée (et pourquoi)

- **`core_bridge` = SEULE porte vers aithos-core/bundle** (avec son annexe
  `store_adapter`). Tout le reste (policy, config, proxy) parle en noms
  d'outils et verdicts. Les évolutions d'API du core s'absorbent là. Le bridge
  ré-exporte l'entropie (`EntropySource`, `OsEntropy`, `SeqEntropy`) pour que
  binaire et tests n'importent jamais le bundle.
- **Trois mandats à l'onboarding** (grants loggés, jamais silencieux) :
  agent (`act.x.mcp.<action>` par outil read), **gateway lui-même**
  (`act.x.gateway.*` — un refus n'est PAS un acte de l'agent, c'est un acte de
  gouvernance du gateway, sous sa propre clé), auditeur
  (`read.gamma#kind=action`).
- **Double mur d'enforcement** : `authorize` (verify_chain + action_covered)
  pour refuser proprement AVANT de relayer ; `log_action` re-vérifie tout à
  l'append (chaîne, révocations, budgets) — le bundle refuse lui-même de
  logger un acte non couvert. **Log-before-relay** : pas d'entrée gamma → pas
  d'appel amont.
- **Contrainte de grammaire absorbée côté gateway** : les actions d'act se
  découpent au DERNIER point (`act.x.<connector>.<action>`), donc les noms
  d'outils MCP pointés (`user.read`) s'aplatissent (`user_read`) dans le
  mandat ; le nom brut reste dans le payload clair (`tool`) ; les collisions
  post-aplatissement sont rejetées à la config.
- **Keyholder** : seeds agent + gateway, zeroize, jamais sérialisés vers la
  console (testé en surface). Persistés entre onboard et run via le store
  (`gateway/keys.json`) — **v1 disque local uniquement ; passer par KMS/keystore
  scellé avant tout store cloud** (le refus S3 de v1 verrouille ça).

## 3bis. Décisions produit du recul multi-Ethos (Mathieu, 2026-07-10)

Gravées en discussion après le MVP ; elles redéfinissent la cible de la
prochaine itération du gateway (le MVP actuel reste la démo mono-Ethos).

1. **Stockage de l'ethos : local OU serveur de référence.** Le serveur n'est
   qu'un backend `Store` de plus (`RemoteStore` à écrire) : requêtes signées
   par la clé d'owner ou de mandataire — le mandat EST la clé d'API, le
   serveur applique le même `covers()` qu'un verifier. Il n'est jamais une
   partie de confiance (intégrité offline) : contrôle d'accès physique +
   disponibilité seulement.
2. **Le container ne crée ni clés d'owner ni mandats — il est provisionné.**
   Au premier boot il génère SA keypair d'agent (la clé ne sort jamais, seule
   la pubkey est publiée) ; l'owner émet les mandats vers cette pubkey depuis
   SES outils (les clés d'owner ne touchent jamais le runtime) ; le container
   reçoit N mandats de N Ethos différents (ex. full "company brand" + Figma
   scopé "UI website designer"). Une clé, N contextes — natif protocole.
   Chaque acte est loggé dans le gamma de l'Ethos dont le mandat le couvre.
   → Remplace l'auto-mint `onboard` du MVP (raccourci de démo assumé).
3. **Création d'un container = identité d'agent + Ethos de travail dédié.**
   L'Ethos de travail (journal de bord, mémoire consolidée de l'agent) est
   créé au provisioning, hébergé typiquement sur le provider (le container
   est éphémère), mais **owné par l'humain** — l'agent n'y est que mandataire
   en écriture. Souveraineté (révocation, succession, crypto-erasure) = owner.
4. **Coffre décentralisé dans les Ethos, jamais chez le LLM.** Les credentials
   tiers (Figma, Gmail…) vivent scellés dans le vault `/x/` de chaque Ethos,
   chiffrés vers la clé du gateway (ligne grantée). Pas de coffre central :
   les accès voyagent avec la donnée, le storage ne lit rien, le gateway
   ouvre en mémoire, le LLM ne voit jamais un secret. Décision ferme : donner
   les credentials au LLM (même chiffrés) casserait enforcement, traçabilité
   et la brique audit — les API tierces ne vérifient pas les mandats Aithos,
   seul le détenteur du credential applique le périmètre.
5. **Deux vues, deux index (précision 2026-07-10).** Attribution par agent :
   déjà garantie par construction (mandat lié à `grantee_pub`, chaque entrée
   signée par la clé de l'agent, `via` = chaîne de mandats) — un mandat n'est
   PAS au porteur ; « donner le même mandat à N agents » exigerait de partager
   la clé privée, que le provisioning rend anti-naturel (clé née dans le
   container, ne sort jamais). Même périmètre pour N agents = N mandats (ou
   délégation récursive, loggée). L'Ethos de travail de l'agent sert de
   **vue par agent** (tout ce que CET agent a fait, tous contextes), duale de
   la **vue par contexte** (gamma de chaque Ethos octroyant) ; l'entrée du
   journal de travail cite l'id de l'entrée autoritaire du contexte + le DID
   de l'Ethos, pour joindre dans les deux sens. Clé dupliquée entre deux
   containers : indistinguable cryptographiquement, mais détectable au gamma
   (budgets à double vitesse, heartbeats incohérents).
   Implémentation du miroir SANS toucher au protocole : l'agent utilise SA
   clé d'identité (pas de « clé de l'Ethos de travail » — cet Ethos est owné
   par l'humain, l'agent y est mandataire) ; l'entrée miroir = convention
   gateway, ex. connecteur `xref` granté sur le journal (`act.x.xref.*`),
   payload libre `{ethos_did, entry_id}`. L'entrée du contexte reste la seule
   source de vérité ; le journal est un index, jamais une preuve autonome.
6. **Gestion de flotte (décidé 2026-07-10).** Personne ne « gère » les clés
   d'agents : 1 container = 1 clé née dedans (jetable — mort/compromission →
   révocation + re-mint, jamais de récupération), 1 entreprise = 1 graine
   maîtresse + succession froide. Inventaire du parc = le log des grants
   (obligatoire) dans le gamma de l'entreprise. **REJETÉ : dériver les clés
   d'agents de la maîtresse** — l'entreprise pourrait forger des signatures
   d'agent → perte de la non-répudiation bidirectionnelle, cœur de l'audit
   externe. La « clé de parc » se fait par mandat d'émission (`issue#depth`)
   à un orchestrateur : mandats par agent atténués, émissions loggées et
   comptées (`max_children`), révocable en bloc — natif étape E.
7. **Trois index d'audit, une vérité.** Gamma du contexte (autoritaire),
   journal de l'agent (vue par agent : miroirs xref + événements « mandat
   reçu », utile quand les contextes appartiennent à des clients), gamma de
   l'entreprise (vue de flotte : grants/révocations/délégations). Jointure
   par `(ethos_did, entry_id)`. Seule l'entrée du contexte fait preuve.
8. **Refus multi-contexte — TRANCHÉ (Mathieu, 2026-07-10).** Le journal de
   l'agent reçoit TOUS les refus, toujours (c'est son histoire). En PLUS,
   si la tentative visait un contexte identifiable ET que ce contexte a
   granté la gouvernance au gateway (`act.x.gateway.*`, comme au MVP), le
   refus s'écrit aussi dans le gamma du contexte (son auditeur doit voir
   les tentatives contre son périmètre). Pas de gouvernance grantée → repli
   journal seul (droit non donné, pas une panne). Rappel mécanique : un
   refus ne peut JAMAIS s'écrire via le mandat de l'action — l'action
   refusée n'est couverte par rien, et le gamma n'accepte que du couvert.
9. **Clés d'owner des journaux d'agents : dérivées de la maîtresse.** Les
   Ethos de travail appartiennent tous à l'entreprise → dériver leurs clés
   d'owner de la graine maîtresse (label = sid d'agent) est sain (aucun
   tiers imputé, pas d'enjeu de non-répudiation — contrairement aux clés
   d'AGENTS, dont la dérivation reste rejetée). Une seule graine au coffre.
   **Dérivation ≠ fusion** : chaque journal reste un Ethos séparé et complet
   (DID, zones, gamma propres) — auditer un agent n'expose RIEN des autres ;
   l'isolation est au niveau du bundle, pas de la clé.
10. **Vision gravée : l'agent n'est pas sa clé.** Âme = son Ethos de travail
    (mémoire consolidée, expertise qui grandit — la valeur à terme) ; corps
    = le container (remplaçable) ; clé = instrument de signature (persiste
    avec lui tant qu'il vit, dans la garde du runner, jamais chez le LLM ;
    remplaçable par re-grant sans perte d'identité — le nom URN et l'Ethos
    portent la continuité). « Jetable » qualifie la clé, jamais l'agent.
    Autonomie déjà native : mandat full = l'agent retravaille son Ethos sans
    permission (profil absentee owner). Changement/départ de maître =
    **succession** (transfert de propriété de l'Ethos, historique intact,
    ancienne époque morte) ; l'« affranchissement » — transférer à l'agent
    la propriété de sa propre mémoire (modèle B2C/marketplace) — est donc
    une opération native du protocole. B2B v1 : entreprise owner.
11. **Pas de triple écriture (précision).** Un acte = 2 écritures (gamma du
    contexte : la preuve ; journal : le miroir xref) et 3 lectures (contexte,
    journal, flotte). Copier chaque acte dans le gamma d'entreprise est
    rejeté : une copie ne prouve rien, et une chaîne unique sérialiserait
    toute la flotte sur une tête. Consolidation d'entreprise = agrégation
    par lecture, asynchrone, hors chemin chaud.

## 4. Reste à faire — phasage v2 (mis à jour 2026-07-10, fin de session)

**Phase B — cœur v2 : ✅ CLOSE (lot 4 soldé le 2026-07-11, 2ᵉ session gw).**
Contrat : `tests/features/gateway-provisioning.feature` (6 scénarios, 6 verts).
- ✅ Lot 0 contrat ; Lot 1 naissance (`keygen`, identité fichier runner 0600,
  hors store, seuls les pubkeys sortent ; `Bridge::open` prend le keyholder) ;
  Lot 2 outillage owner (`owner-init-journal` — clés d'owner dérivées §9,
  stylo xref à l'agent ; `owner-init-context` ; `owner-grant-context` vers
  une PUBKEY : read tools + gouvernance + auditeur ; grants loggés ;
  `GatewayStore` clonable, mem partagé pour les tests).
- ✅ Lot 3 (2026-07-11) : config v2 `contexts:`+`journal:` (formes mono/multi
  exclusives fail-closed, collisions inter-contextes rejetées — 16 tests
  unitaires), `Runner` multi-Ethos dans core_bridge (keyholder partagé
  `Arc` entre N bridges — custody intacte ; xref `act.x.xref.ref` payload
  clair `{ethos_did, entry_id, tool}` après chaque acte ; refus routés
  §3bis.8 : journal TOUJOURS + contexte quand l'outil en désigne un),
  `McpRouter`/`process_multi` dans proxy_mcp (log-before-relay ×2 : acte
  contexte puis xref journal, tout échec d'append refuse ; relay vers
  l'upstream DU contexte), `run` multi dans le binaire (mono inchangé).
  **Simplification v1 assumée** : le routeur multi ne sert que
  `tools/call` — `initialize` répond statique, `tools/list` agrège les
  NOMS d'outils déclarés (inputSchema objet ouvert, pas de proxy de
  schémas), toute autre méthode → -32601 (passthrough complet : Phase D).
  Suite : 11 scénarios / 57 steps, 16 unit, 4 CLI, 1 e2e, clippy clean.
- ✅ Lot 4 (2026-07-11, clôture Phase B) : `tests/owner_surface.rs` (3 tests
  assert_cmd, binaire réel — certs écrits et `grantee.pubkey` = LA pubkey
  fournie au grant, grants loggés dans le gamma, auditor_seed_hex montrée
  UNE fois à côté du warning « STORE COLD », master seed jamais échoé,
  runner seeds jamais côté owner, warning DEV ONLY, inputs malformés
  fail-closed) ; `audit-export --context <name>` (requis en config multi,
  refusé en mono, contexte inconnu refusé — chaque contexte a SON gamma,
  SON mandat et SA seed d'auditeur ; mono inchangé) ; `tests/e2e_multi.rs`
  (1 e2e réseau : keygen → provisioning owner par le binaire → `run` multi
  → JSON-RPC sur vraies sockets : chaque read routé vers SON faux MCP —
  réponses distinctes observables sur le fil —, write refusé -32001,
  inconnu default-deny, rien d'autre ne traverse → gammas vérifiés (acte
  dans le contexte couvrant seulement, xref joignable dans les deux sens,
  refus routés §3bis.8 : contexte+journal vs journal seul) → audit-export
  par contexte + refus hors périmètre kind=grant). **Leçon notée** : la
  seed d'auditeur ne gate PAS la lecture des en-têtes clairs (offline =
  squelette lisible pour qui tient les fichiers ; la moitié certificat
  gate la REQUÊTE, la seed ouvre les corps scellés) — un « refus
  cross-contexte par seed » n'existe donc pas par construction, ne pas
  le re-tester.
  Suite finale Phase B : 11 scénarios / 57 steps, 16 unit, 4 CLI,
  3 owner surface, 2 e2e réseau (mono + 2 contextes), clippy clean.

**Phase C — la boucle « agent qui vit » (OUVERTE 2026-07-11, proxy_llm ✅).**
- ✅ `proxy_llm` OpenAI-compat (2026-07-11, contrat
  `tests/features/gateway-inference.feature` — 7 scénarios verts, écrits
  AVANT le code et committés seuls) : le gateway détient la clé API
  (config `llm:` v1, multi-shape uniquement ; vault `/x/` = cible §3bis.4,
  même couture), **impose le modèle** (le choix de l'agent est écrasé),
  lit l'usage RÉEL du provider, logge une entrée `inference` par appel
  dans le JOURNAL (méta seulement : provider/model/tokens_in/tokens_out/
  budget_ref — JAMAIS le prompt), budgets tokens F+. Mécanique :
  - **Pen d'inférence SÉPARÉ** : `owner-init-journal --token-budget N`
    mint un 3ᵉ mandat vers la pubkey agent (`act.x.llm.*`, constraints
    `budgets: [{id: "llm", token_budget: N}]`) — séparé exprès : un
    mandat à budgets force chaque entrée à citer un budget_ref, le stylo
    xref doit rester budget-free. Pas de pen → pas de LLM (fail-closed).
  - **Robinet fail-closed des deux côtés du provider** : avant = headroom
    (pen présent, chaîne valide, spent < budget) sans round-trip ; après
    = usage absent OU dépassement à l'append → complétion RETENUE +
    refus journal (`llm.chat`, usage_missing / log_append_refused).
  - **Un seul Runner partagé** (`Arc<Mutex<Runner>>` entre McpRouter et
    LlmProxy — jamais deux bridges sur un même store, risque de fork) ;
    endpoint `/v1/chat/completions` sur le même listener que `/mcp`.
  Suite : 18 scénarios / 91 steps, 19 unit, 4 CLI, 3 owner surface,
  2 e2e réseau, clippy clean.
- ✅ **Lot C2 — outils journal MCP : CLOS (2026-07-12 soir, 4ᵉ session
  gw), les 5 décisions VALIDÉES par Mathieu, l'impl vit sur pass L.**
  Chronologie : contrat v1 @wip (`5b9ff86`, hypothèse notes-gamma, core
  sans écriture de section côté agent) → **pass L** (écritures déléguées
  circle, session parallèle : `section_add/rewrite/delete_as_agent`,
  `GrantSpec.verb`, `deliver_zone_line`, `log_delegated_mutation` —
  revalidée depuis le disque et committée `939accb`, voir
  `docs/2026-07-12-delegated-writes.md`) → contrat v2 sections
  (`a39eb91`) → impl verte + dé-tag (`5c77753`, 11 scénarios). Les
  décisions gravées :
  1. **Périmètre** : write + search. Schémas fail-closed — write
     `{text requis non-vide, title?, tags?[]}`, search `{query?, tag?,
     limit? (défaut 20, max 100)}`, champs inconnus rejetés (parser ET
     inputSchema `additionalProperties: false`).
  2. **Cible** : UNE section scellée par écriture dans `circle:memory/`
     (nom technique frais `n-<hex>`, title/tags humains CLAIRS à
     l'index, corps scellé au repos) ; la trace = `section.add` délégué
     à corps scellé. Le dossier `memory` est préparé par
     `owner-init-journal` (`ensure_folder` + `publish`, le Given
     pass-L) — un périmètre append fait pousser du contenu, jamais
     l'arbre.
  3. **Pen** : mandat mémoire DÉDIÉ (`append.circle#dir=<sid memory>`)
     vers la pubkey agent, minté à `owner-init-journal` (toujours,
     imprimé `memory_mandate`) + `deliver_zone_line` (moitié physique
     §04.3) — un pen par usage, révocable indépendamment du stylo xref ;
     `append` crée ET lit (lattice §04.2), jamais rewrite/delete (la
     mémoire v1 est append-only). Journaux antérieurs sans pen → refus
     fail-closed (précédent LLM) ; exercé par scénarios « legacy »
     (state.json délesté du pen avant l'ouverture du bridge).
  4. **Exposition** : noms pointés `journal.write`/`journal.search` ;
     préfixe `journal` RÉSERVÉ dans TOUTE tool map (mono et multi :
     `journal`, `journal.*`, `journal__*` → config rejetée), miroir de
     HUB-MCP §5 ; `tools/list` sert les natifs avec leurs VRAIS schémas
     (les outils de contexte gardent l'objet ouvert honnête jusqu'au
     hub). Interception AVANT `resolve` dans `process_multi` — jamais
     relayés ; refus §3bis.8 journal seul (aucun contexte identifiable).
  5. **Search** : match sur l'index CLAIR seulement (name/title/tags,
     sous-chaîne case-insensitive pour `query`, égalité exacte pour
     `tag` ; la frontière de lisibilité — le gateway tient les
     fichiers), antéchrono (ordre d'insertion de l'index inversé — les
     sids ne sont PAS time-ordered, entropie pure) ; les corps sont
     ouverts pour les SEULS hits rendus (≤ limit), chaque ouverture =
     `read_section_as_agent` + UNE entrée `ethos.read`
     (`log_read_as_agent`) ; une ouverture non journalisable fait
     échouer TOUT le recall ; zéro match = zéro ouverture = zéro
     entrée. Le full-text corps (ouvrir N notes = N lectures loggées)
     reste un choix futur explicite.
  Outillage owner ajouté : `journal_notes_view` (squelette clair) et
  `owner_read_journal_note` (souveraineté §3bis.3 : l'owner relit la
  mémoire de son agent avec ses clés dérivées — le step « the owner
  reads back » du contrat). Bridge : `memory_chain` chargé de
  `state.json` (champ additif `memory_mandate`, absent = refus).
  NOTE pass-L à suivre côté gateway : `record_section_add/rewrite/
  delete` (chaîne AGENT générique) restent la couture des écritures de
  CONTEXTE futures — non utilisés par C2 (qui passe par le pen mémoire
  dédié), non exercés par un scénario gateway encore.
- ✅ **Surfaces C soldées (2026-07-12, 3ᵉ session gw).** e2e réseau llm :
  `tests/e2e_llm.rs` (`3460168`) — parcours binaire réel (keygen →
  provisioning `--token-budget 700` → `run` multi+`llm:`), faux provider
  OpenAI-compat sur vraie socket : bearer VU SUR LE FIL (et seulement
  là), le modèle imposé écrase le choix de l'agent, 1 entrée `inference`
  à l'usage RÉEL du provider (400/300, budget_ref `llm`), 2ᵉ appel
  refusé AVANT le provider (700/700, hit count 1, refus
  `llm.chat`/`mandate_denied` au journal), ni prompt ni credential dans
  AUCUN fichier de store (journal ET contexte). Owner surface
  `--token-budget` : 4ᵉ test d'`owner_surface.rs` (`e78f318`) —
  `inference_mandate` imprimé, cert vers LA MÊME pubkey agent portant
  `budgets [{id: llm, token_budget}]`, stylo xref budget-free vérifié,
  3ᵉ grant loggé ; sans flag : ni pen ni 4ᵉ ligne. « e2e multi étendu au
  `llm:` » : absorbé — e2e_llm porte déjà la forme multi
  (contexte+journal+llm).
- ⬜ Creds provider vers le vault `/x/` (cible §3bis.4 ; la config v1 est
  la couture temporaire assumée).
- **Contexte produit** : `docs/EXPLORATION-DESKTOP-GATEWAY.md`
  (2026-07-11, piste NON tranchée) projette le gateway en hôte desktop —
  proxy_mcp/McpRouter/proxy_llm déjà verts y sont les briques du MVP ;
  ses chantiers neufs (`proxy_web`, `RemoteVault`, packaging desktop, UX)
  rejoignent la Phase D si Mathieu tranche. Ses 4 questions ouvertes (§9)
  sont à lui.

**Phase H — hub MCP gouverné : ✅ CLOSE (2026-07-13, H0→H4 + H2b).**

- ✅ **H0 — contrat** (`dd680ae`, écrit sur la branche dédiée
  `codex/gateway-hub-h0`, **mergé ff sur `feat/obligations`** puis
  amendé `42e378e` — 5ᵉ session gw) :
  `tests/features/gateway-hub.feature`, 11 blocs `@wip` écrits avant le
  code. Le contrat couvre enrollment +
  pin intégral owner-approved, `tools/list` limité aux outils couverts et
  reconstruit sans l'amont, surface v1 `tools/*` seulement, un serveur
  partagé entre deux Ethos sans ambiguïté de preuve, write connu caché
  mais refusé précisément, drift fail-closed + gouvernance, re-enrollment
  avec nouveau mandat et révocation politique de l'ancien, réservation de
  `journal`, collision d'un outil entre contextes et collision après
  aplatissement. Il ne tranche ni le stockage du manifeste ni la cadence
  du contrôle de drift. Retouches de review (`42e378e`) : `gateway`
  réservé à côté de `journal` dans l'Outline ; l'ambiguïté
  d'aplatissement INTER-serveurs épinglée (`a`+`b__c` et `a__b`+`c`
  exposent tous deux `a__b__c` → rejet nommé ; si H1 préfère interdire
  `__` dans les ids de serveurs, ce scénario se réécrit en rejet de
  charset — à trancher là) ; le refus du write connu route vers « the
  context that knows the tool » (l'outil n'est justement pas granté —
  nuance map interne de la décision 3.2). Validation : gateway Cucumber
  historique **29 scénarios / 145 steps verts** ; H0 parsé puis ignoré
  par `@wip` (sonde : dé-tagger UN scénario fait apparaître 5 features /
  30 scénarios, 1 skipped — le fichier est bien parsé, pas contourné).
- ✅ **H1 — config v3 `servers:`** (`6b580ff`) : ressources serveur de première
  classe, outils référencés par `(server, tool)`, formes exclusives et toutes les
  ambiguïtés H0 rejetées fail-closed ; legacy v1/v2 compatible.
- ✅ **H2 — enroll owner-side** (`f915d34`) : capture stricte `tools/list`,
  approbation explicite, JCS/SHA-256, manifeste scellé sous `/x/<server>`, ligne
  gateway, mandat exact et grants journalisés ; surface binaire réelle testée.
- ✅ **H3 — runtime** (`4fc4b4d`) : pins ouverts par le
  gateway, liste couverte reconstruite sans amont, serveur partagé, relais brut,
  log par Ethos + xref, contrôle drift et bearer config. Cinq scénarios runtime
  détaggés ; suite finale 40/195.
- ✅ **H2b — re-enrollment** (`aa890b8`) : `owner-enroll-server --replace`, même pubkey agent
  obligatoire, remplacement du pin sous la clé vault existante, nouvel équipement,
  révocation owner des anciens mandats agent/gateway/auditeur ; nouveau schéma servi
  après réouverture. Dernier scénario H0 détaggé : hub **zéro `@wip`**.
- ✅ **H4 — e2e réseau hub** : `tests/e2e_hub.rs`, vrai binaire + deux MCP sur
  sockets localhost, trois Ethos, serveur partagé, bearer observé sur le fil et
  absent des stores, noms bruts, gammas/xrefs/audit-export, restart fail-closed
  après drift de description.

**Phase D — industrialisation.** Args scellés (quick win : `log_action` les
accepte, manque `grant_audit_line` + flag), `tools/list` filtré, passthrough
SSE/streaming complet, container/pod + egress lockdown (deployment doc),
`RemoteStore` signé puis S3, ops (révocation, dashboard). ~~Test d'intégration
HTTP bout en bout~~ — FAIT (`e2e_http.rs`, 2026-07-10).

**Opérationnel local « vision complète » = B + proxy_llm minimal de C.**
Note : H (racines gamma, preuves de complétude — en cours côté core) upgrade
l'export d'audit dès son merge, sans travail gateway.

## 5. Env sandbox — leçons du 2026-07-10 (s'ajoutent au HANDOFF §5 du core)

- **CARGO_HOME dédié sur le volume** : `rust/target-linux/cargo-home` (les
  caches /tmp appartiennent à `nobody` après recyclage VM et /tmp est plein).
  Seedé depuis `/tmp/cargo2/registry` (cache+index lisibles).
- **CARGO_TARGET_DIR SÉPARÉ PAR SESSION** : `rust/target-linux-gw` pour le
  gateway. NE PAS partager `target-linux` avec la session core : les flocks
  cargo sont inopérants sur le montage FUSE → deux cargo concurrents se
  corrompent mutuellement (rmeta/dep-info mutilés, E0463 en cascade).
- **`timeout 40` tue cargo en pleine écriture** : après chaque kill, purger
  les artefacts du crate en cours (`rm deps/<crate>-*`), sinon la corruption
  empoisonne les builds suivants. Les GROS crates (tokio ~34 s) se buildent
  seuls : `cargo build -j 1 -p tokio` dans une tranche pleine.
- SIGBUS rustc sporadiques (mmap sur FUSE) : retry, puis purge ciblée.
- **2026-07-11 (2ᵉ session gw) : profil VM HYBRIDE, les recettes ci-dessus
  sont mortes.** Egress coupé (000 partout) ET unlink interdit SUR LE
  MONTAGE (cargo meurt en `failed to remove …rcgu.o`) — mais unlink OK
  hors montage (/tmp) et la toolchain 1.96.1 de la session du 11/07
  respire encore dans `/tmp/rustup` (inutile sans réseau ni target
  writable). → **Protocole cloud+janitor du HANDOFF §5 core, à la
  lettre** : `git archive HEAD` sur la VM → tar dans `_transfer/` du
  montage (device_stage_files ne lit QUE sous le montage) → sha256 croisé
  → build/test dans le conteneur cloud (rustup 1.96.1 minimal + clippy +
  rustfmt, `CARGO_INCREMENTAL=0`, TARGET_DIR dédié, suite gateway ~2 min
  à froid) → retour des fichiers modifiés par device_commit_files + `cp`
  (jamais tar -x sur le montage) → commits git sur la VM avec janitor des
  locks (`mv .git/*.lock` avant chaque commande qui écrit, jamais de
  `git status` intercalé). Tester les DEUX sondes (egress + unlink sur le
  MONTAGE, pas /tmp) avant de choisir le profil.
- **2026-07-12 (3ᵉ session gw) : profil hybride CONFIRMÉ** (egress 000,
  unlink interdit sur le montage — sondes refaites), protocole
  cloud+janitor déroulé sans accroc (rustup 1.96.1 pinné, suite gateway
  ~2,5 min à froid, sha256 croisés à chaque transfert, dans les deux
  sens). Deux leçons neuves : (a) le PONT DESKTOP peut flapper
  (déconnexions/reconnexions en boucle) — 3 AskUserQuestion coupés en
  plein vol → appliquer le mode absent du brief (defaults @wip
  committés, zéro impl des points non tranchés) au lieu de re-tenter en
  boucle ; les transferts de fichiers, eux, passent entre deux flaps.
  (b) au commit, les warnings `unable to unlink .git/objects/*/tmp_obj_*`
  et `HEAD.lock` sont COSMÉTIQUES (le commit aboutit) ; janitoriser
  `HEAD.lock` avant la commande git suivante, comme d'habitude.
- **2026-07-13 (6ᵉ session gw) : profil local utilisable.** Egress toujours
  coupé, mais unlink fonctionne sur le montage et la toolchain locale est
  `rustc/cargo 1.95.0`. Builds avec `CARGO_INCREMENTAL=0` et target isolé
  `rust/target-gw-codex-20260713`. Les E2E ont seulement nécessité l'autorisation
  sandbox d'ouvrir des sockets localhost. `.git/objects/maintenance.lock` daté du
  11/07 reste présent et n'a bloqué ni staging ni commit ; ne pas le supprimer à
  l'aveugle.

## 6. ⚠ Git : sessions parallèles, un seul working tree

La session core (G/move-as-rotation) et la session gateway partagent le même
working tree. Le passage à `feat/gateway` a déplacé HEAD sous la session core :
ses commits `e721fce` (spec+feature move) et `f613a93` (vector G3) ont atterri
SUR `feat/gateway`, intercalés entre les commits gateway. Fichiers disjoints,
rien ne se conflicte, mais à réordonner à la fin (cherry-pick vers feat/f-plus
ou merge global — décision Mathieu). Éviter à l'avenir : `git worktree add`
par session, ou une seule session git-active à la fois.

Même situation le 2026-07-11 : les commits du lot 3 (gateway) ont été posés
sur **`feat/obligations`** (branche active de la session core du moment,
consigne : ne jamais switcher). Fichiers disjoints (crate gateway + ce doc) ;
à réordonner avec le reste au moment du merge global. Les commits du lot 4
(clôture Phase B, 2ᵉ session gw du 2026-07-11) suivent la même consigne :
posés sur `feat/obligations`, staging sélectif, fichiers disjoints du core.

Depuis le soir du 2026-07-11 : **la session core est terminée** (plan 0→K
complet, `a58ab4d`) — plus de cargo concurrent ni de commits intercalés à
craindre, mais la consigne reste : `feat/obligations`, jamais de switch,
staging sélectif (le merge des branches est la décision post-plan n°2 de
Mathieu). Scories untracked assumées sur le volume : `_transfer/` (tar de
transfert cloud), `_gitjunk/` (locks janitorisés), `_to_delete/` (débris de
sondes) — suppression impossible depuis la VM, ignorer.

Session du 2026-07-12 (3ᵉ gw) : mêmes consignes appliquées — commits
`5b9ff86` (contrat C2 @wip), `e78f318` (owner surface `--token-budget`),
`3460168` (e2e réseau llm), `f87f072` (handoff), tous sur
`feat/obligations`, staging sélectif, fichiers 100 % gateway/docs.

Session du 2026-07-12 soir (4ᵉ gw) : l'arbre est arrivé SALE — la pass L
(session sandbox parallèle) avait déposé ses 10 fichiers sur le disque
sans commit (bridge coupé chez elle, voir
`docs/HANDOFF-2026-07-12-pass-L.md`). Protocole appliqué : overlay
HEAD+sale rapatrié dans le cloud, workspace revalidé (203/826 bundle,
tout vert), 2 hunks fmt manquants appliqués, puis **commit dédié pass L
`939accb`** (11 fichiers, décision Mathieu Q1) AVANT tout travail
gateway. Ensuite : `a39eb91` (contrat C2 v2 sections), `5c77753` (impl
C2 verte, 8 fichiers gateway), plus le commit de ce handoff. Leçon : une
session qui livre du code par le chat sans bridge doit le signaler en
tête de SON handoff (fait ici) — et la session suivante REVALIDE depuis
les fichiers du disque avant de committer, jamais sur la foi du sandbox
d'origine. `docs/HUB-MCP.md`, `docs/EXPLORATION-DESKTOP-GATEWAY.md` et
`docs/STANDARDS-COMPAT.md` (apparu en cours de session) restent
UNTRACKED — à Mathieu de décider s'ils se committent.

Session du 2026-07-12 midi (5ᵉ gw) : mission review de la branche
`codex/gateway-hub-h0` (H0 écrit hors session Cowork, 2 commits posés
sur `ae0fb3a`). Review livrée au chat, puis sur demande de Mathieu :
**merge = ff MANUEL sans worktree git** (`git merge --ff-only` aurait
fait des unlink() de worktree, interdits sur le montage) — écriture des
2 fichiers depuis le ref (`git show` → tmp `_transfer/` → mv
par-dessus), `git add` sélectif, `git update-ref -m` → HEAD `082f5a2`,
arbre/index/HEAD vérifiés alignés. Puis retouches contrat `42e378e` et
ce handoff, transferts sha256-croisés dans les deux sens, janitor avant
chaque commande git écrivante, warnings tmp_obj/HEAD.lock cosmétiques
confirmés. La branche `codex/gateway-hub-h0` reste posée sur `082f5a2`
(la supprimer = décision Mathieu). Scorie ajoutée : `_transfer/
hub-082f5a2.tar` (tar de review).

Session du 2026-07-13 (6ᵉ gw) : reprise directe sur `feat/obligations`, jamais de
switch. HEAD d'arrivée `f743cd6` (le document STANDARDS avait entre-temps été suivi
par Mathieu). Commits sélectifs : `6b580ff` H1, `f915d34` H2, `4fc4b4d` H3,
`088b82f` H4, `aa890b8` H2b ; ce paragraphe finalise ensuite le handoff.
`docs/HUB-MCP.md` a été inclus avec autorisation explicite. Les scories `_gitjunk/`,
`_to_delete/`, `_transfer/` et `docs/EXPLORATION-DESKTOP-GATEWAY.md` sont restées
intactes et non stagées.

Session du 2026-07-15 (7ᵉ gw, coffre Vault, profil cloud+janitor) : arrivée
sur `e9d2a8d` avec un `index.lock` du 13/07 traînant — janitorisé vers
`_gitjunk/` comme le reste. Transfert par `git archive HEAD` →
`_transfer/aithos-src-20260715.tgz` (un premier tar naïf de 465 Mo a timeout
le pont : target pris par un pattern d'exclusion défaillant — écrasé par
l'archive propre), build/test cloud rustc 1.95.0, retours
device_commit_files **par tranche** (16 payloads, sha256 croisés un à un)
pour que chaque commit Mac porte l'état exact de sa tranche : `9dd81fc`
contrat V0 @wip seul, `ea224d3` V1, `34dfd22` V2, `916ecb3` V3, puis le
commit docs (ce paragraphe, l'état express, HUB-MCP §8 et le handoff DONE).
Warnings `tmp_obj` toujours cosmétiques. L'input
`HANDOFF-GATEWAY-VAULT-FINALIZATION-2026-07-15.md` reste untracked (décision
Mathieu). Scories intactes.

Session du 2026-07-15 soir (8e gw, démo Léa, même profil cloud+janitor) :
scénario de référence validé par Mathieu puis commits sélectifs
`6ba28d6` (doc scénario), `190d6b4` (4 contrats @wip seuls), `0e59e91`
(lot W), `56d2a14` (lot P), plus le commit docs de ce paragraphe et du
handoff K-D. Transferts fichier-par-fichier sha256-croisés, janitor
habituel, warnings tmp_obj cosmétiques. Leçon neuve : Cucumber passé en
séquentiel (voir état express).

Session du 2026-07-15 nuit (9e gw, lots K+D, même profil cloud+janitor) :
arrivée sur `4563f72`, transfert `git archive HEAD` →
`_transfer/head-4563f72.tar` (sha256 croisé), build/test cloud rustc
1.95.0, retours fichier-par-fichier device_commit_files sha256-croisés
par tranche : `b2f5b69` (lot K, 7 fichiers), `0db670e` (lot D, 6
fichiers dont `tests/e2e_demo_lea.rs` neuf), puis le commit docs
(runbook `DEMO-LEA.md`, cet état express, ce paragraphe, le handoff
DONE). Janitor des locks avant chaque commande git écrivante, warnings
tmp_obj cosmétiques, scories intactes. Leçon d'environnement : le pont
desktop s'est déconnecté en cours de session (précédent de la 3ᵉ gw) —
les tranches K et D étaient déjà committées ; la tranche docs a attendu
la reconnexion, le travail cloud a continué entre-temps.

Session du 2026-07-16 (10ᵉ gw, surface mandats M0→M2, profil
cloud+janitor) : arrivée sur `fc86ed1`, gate M0 recueilli par
AskUserQuestion AVANT tout contrat (les six recos confirmées), sondes
refaites (egress 000, unlink DENIED sur le montage — débris laissé dans
`_to_delete/` —, pas de toolchain locale sur la VM), transfert
`git archive HEAD` → `_transfer/head-fc86ed1.tar` (sha256 croisé
`7ba1a41f…`), build/test cloud rustc 1.95.0, **baseline revalidée À
L'IDENTIQUE avant toute modif** (62/4/88-473/6e2e/5 owner gw ; 97 +
203/826 core). Retours device_commit_files fichier-par-fichier
sha256-croisés par tranche : `aa02353` (M1 — les 3 contrats SEULS,
sonde de parse détag/re-tag exécutée dans le cloud des deux côtés),
`f8cbc88` (M2 — 6 fichiers gateway), puis le commit docs (état express,
ce paragraphe, le handoff M3). Janitor habituel (un `HEAD.lock`
janitorisé avant le commit M2), warnings tmp_obj cosmétiques confirmés,
jamais de `git status`, scories intactes. Les nouvelles surfaces M2
sont 100 % additives : aucun fichier du chemin chaud démo modifié hors
`core_bridge.rs` (ajouts purs), `main.rs` (commande nouvelle) et
`cucumber.rs` (steps + 2 champs de monde).

Session du 2026-07-16 nuit (12ᵉ gw, lots G2+G6, profil cloud+janitor) :
arrivée sur `6fdfe3c`, sondes refaites (egress 000, unlink DENIED sur le
montage — débris `_to_delete/probe-unlink-20260716-s12.txt`), tar du working
tree → `_transfer/aithos-core-src-20260716-g2g6.tgz` (sha256 croisé
`75531ad7…`), baseline revalidée À L'IDENTIQUE en cloud avant tout travail.
Commits sélectifs sha256-croisés fichier par fichier : `3b451ae` (les 2
contrats SEULS), `d17d77b` (G2, 4 fichiers), `1350e20` (G6, 7 fichiers),
puis le commit docs (ce paragraphe + l'état express). Gates réels exécutés
dans le conteneur cloud : MCP Inspector + Claude Code contre le vrai binaire
en loopback (G2), remplissage des zones par CLI + grant à chaud + lecture
Claude (G6). Janitor habituel (HEAD.lock janitorisé), warnings tmp_obj
cosmétiques, jamais de `git status`, scories intactes (+ le tar de cette
session). Le pont desktop a flappé deux fois en cours de session (précédent
3ᵉ/9ᵉ gw) — le travail cloud a continué, les questions ont été reposées à la
reconnexion. Constat d'arbre : `vectors/README.md` porte une modification
non commitée de la piste P (annexes P0, session AWS parallèle) — non
touchée, décision Mathieu. Docs untracked mis à jour sur le disque sans les
committer (statut inchangé, décision Mathieu) : `HANDOFF-GATEWAY-HUB.md`
(état express 12ᵉ) et `HANDOFF-GATEWAY-G2-G6-DONE-2026-07-16.md` (le handoff
de cette session).

Session du 2026-07-17 (13ᵉ gw, lot G3 — l'AS OAuth `gateway_as`, profil
cloud+janitor) : arrivée sur `22a67c4`, sondes refaites (egress 000, unlink
DENIED sur le montage, pas de toolchain VM), tar du working tree →
`_transfer/aithos-core-src-20260716-g3.tgz` (sha256 croisé `8dad1625…`),
baseline revalidée À L'IDENTIQUE en cloud avant tout travail (82 unit après
la tranche config, 63 à l'entrée). Commits sélectifs sha256-croisés fichier
par fichier : `4eb1b39` (le contrat `gateway-oauth.feature` @wip SEUL, sonde
de parse détag/re-tag exécutée cloud), `9610fe1` (impl G3, 9 fichiers
gateway, 33 scénarios détaggés), puis ce commit docs. **Gate réel exécuté
dans le conteneur cloud** : vrai binaire `run` avec `as:` en loopback, client
OAuth générique scripté (python/requests, 20 checks verts) obtient un token
et appelle `tools/list` ; MCP Inspector CLI liste à travers l'endpoint
OAuth-protégé avec le bearer, refusé sans ; clé d'adapter 0600 née au 1er
run, absente des stores/logs ; émission journalisée (`act.x.gateway.oauth_issue`,
nomme le client, zéro octet de token). Janitor habituel (HEAD.lock
janitorisé), warnings tmp_obj cosmétiques, jamais de `git status`, staging
sélectif (les 9 fichiers gateway nommés un à un — P jamais stagé). Le pont
desktop a flappé plusieurs fois (précédent 3ᵉ/9ᵉ/12ᵉ) — travail cloud
continué, transferts passés entre deux flaps, AskUserQuestion reposé UNE fois
à la reconnexion. Constat d'arbre : la piste P laisse `rust/Cargo.toml`,
`rust/Cargo.lock`, `vectors/README.md` sales — non touchés, disjoints,
jamais stagés. Trois scripts du test navigateur déposés dans
`_transfer/g3-browser/` (untracked, aide Mathieu). Docs untracked mis à jour
sur le disque (décision Mathieu) : `HANDOFF-GATEWAY-HUB.md` (état express
13ᵉ) et `HANDOFF-GATEWAY-G3-DONE-2026-07-17.md` (le handoff de cette
session).

