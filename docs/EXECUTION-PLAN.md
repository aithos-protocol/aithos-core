# Plan d'exécution — implémentation de référence aithos-core

> Document de travail (interne, FR). La spec `spec/` est la source de vérité ;
> ce plan ordonne sa construction pour avancer **sans revenir en arrière**.

## Principes

1. **Vecteurs d'abord (TDD).** Chaque étape commence par ses vecteurs JSON dans
   `vectors/`, valeurs attendues générées indépendamment du code Rust quand
   c'est possible (Python blake3/PyNaCl, comme A1). Le test rouge précède le code.
2. **BDD — le contrat comportemental de chaque phase.** Avant le développement
   de chaque phase, on co-écrit son fichier Gherkin (`features/*.feature`, en
   anglais, exécuté par `cucumber-rs`) : c'est là qu'on définit *ce qu'on peut
   attendre*, de manière flexible — un scénario s'amende en une ligne avant que
   le code existe. Les phases suivantes ÉTENDENT les features, ne les
   réécrivent jamais : le dossier `features/` est à la fois le garde-fou
   anti-régression, la documentation vivante du protocole (lisible telle
   quelle, atout build-in-public), et — accumulé — le test K final. Pyramide :
   vecteurs = vérité au niveau des octets, unitaires au milieu, Gherkin =
   acceptation comportementale. On ne gherkinise pas les unités.
3. **DDD-lite.** La spec est le langage ubiquitaire : chaque type du code porte
   exactement le nom de son concept dans la spec (Mandate, Perimeter, NodePath,
   Edition, GammaEntry, …) ; les frontières de crates/modules suivent les
   chapitres. Aucun concept de code sans concept de spec, et réciproquement.
4. **Checkpoint manuel CLI par étape** (non bloquant). Chaque étape livre son
   verbe CLI et une mini-checklist copier-coller « à la main » pour Mathieu —
   idéalement dérivée des scénarios Gherkin de la phase. La CI ne dépend jamais
   de ces checks ; ils servent à *sentir* le produit.
5. **Une étape = une PR mentale.** Feature co-écrite → vecteurs verts (natif +
   wasm32 check) → scénarios cucumber verts → clippy/fmt verts → commit(s) →
   validation de Mathieu → étape suivante.

## Décisions figées d'avance (anti-retour-arrière)

Ces choix conditionnent les octets signés ; on les fige à l'étape 0 pour ne
jamais avoir à re-signer/re-générer les vecteurs :

- **Encodage wire des clés publiques** : multibase `z…` (base58btc,
  multicodec ed25519-pub `0xed01` / x25519-pub `0xec01`), style `did:key` —
  requis par les certs (§04.1) et le DID doc (§01.4). L'hex reste réservé aux
  vecteurs internes.
- **JCS RFC 8785** pour tout JSON signé/haché ; une seule implémentation
  (crate `serde_jcs`), testée contre les exemples de la RFC.
- **AAD** : convention §00.3 figée (labels NUL-séparés), constantes dans
  `derive.rs`, jamais de littéraux épars.
- **Horloge et RNG injectés** : toute fonction qui a besoin de `T` ou d'aléa
  les prend en paramètre. Aucune exception, dès maintenant.
- **Taxonomie d'erreurs fail-closed** : un variant nommé par rejet de la spec ;
  les tests de fail-closed s'écrivent contre les variants, pas contre des strings.
- **Schéma de vecteur** : `{vector, description, inputs…, expected…}` +
  vecteurs négatifs `{…, must_fail: "<variant>"}`.

## Les étapes

### 0 — Conventions (½ étape)
Figer les décisions ci-dessus : module `wire.rs` (multibase/multicodec),
`serde_jcs` + tests RFC 8785, schéma des vecteurs documenté dans
`vectors/README.md`, **harnais cucumber-rs** (runner + premières step defs
vides, dossier `features/`).
**Manuel :** —. **Done :** tests d'encodage verts, `cargo test` exécute cucumber.

### A — Genèse & identité (spec 01)
Déjà scaffoldé : valider A1 (`cargo test`), puis clé de succession (genèse,
DID doc), DID doc complet (§01.4) signé root, format `did:aithos:z…`.
**CLI :** `init` (existe), `did show`.
**Manuel :** `aithos-core init --seed-hex 000102…1f` → comparer aux valeurs
de `vectors/a1-genesis.json` ; `init` sans seed deux fois → clés différentes.
**Done :** A1 + A2 (DID doc) verts, natif + wasm.

### B — Dérivation & chemins (spec 02.1–02.5)
Chemins canoniques sid (fait), labels `d/ s/ t/`, dérivation profonde,
vecteur B2 (zone → dossier → dossier → section), clés de vues tag.
**CLI :** `node key <path> --seed-hex` (debug) — prouve le déterminisme.
**Manuel :** dériver deux fois le même chemin → même clé ; deux sids voisins
→ clés sans rapport.
**Done :** B2 vert ; propriété « a/b ne couvre jamais a/bc » testée (fait).

### C — Scellés & headers (spec 03)
ECIES multi-lignes X25519-HKDF-AEAD, purpose-AAD, header.json (key_versions,
ligne owner I3), seal/open owner et grantee, wraps (tag + up-link : même
primitive). Vecteur C1 seal/open, C2 wrap.
**CLI :** `header seal|open` sur fichiers de test.
**Manuel :** sceller vers deux destinataires, ouvrir avec chacun ; corrompre
un octet → rejet.
**Done :** C1–C2 verts ; I3 fail-closed (header sans ligne owner → invalide).

### D — Bundle minimal & éditions (spec 02.3, 02.6–02.7) ← avancé exprès
Layout disque §02.3, manifest JCS signé, chaîne d'éditions (height/prev_hash),
`Store` fs, index circle clair / self opaque + descripteurs scellés.
Pas encore : merge/fork (→ étape I).
**Les features BDD deviennent end-to-end ici** (elles pilotaient le core pur
en A–C) : init → créer dossiers/sections → publier édition → relire et vérifier.
**CLI :** `folder add`, `section add|edit`, `zone show`, `edition publish|verify`.
**Manuel :** créer `circle/projets/perso/note1` taguée `toto`, publier,
`zone show circle`, vérifier l'édition ; inspecter `e/self/` à l'œil nu →
aucun nom visible.
**Done :** D1 (édition) vert ; e2e v1 vert ; le bundle d'un `self` ne fuit rien.

### E — Mandats & verifier (spec 04, 05)
Le gros morceau. Document de mandat (kex_pubkey = ed2x vérifié), grammaire
`dir=/tag=/id=` + conjonctions, `covers()` complet (verbes, sélecteurs),
atténuation par lien (§05.3), algorithme verifier §04.5 (T injecté), grant =
cert + lignes (§04.3). Contraintes tier V structurelles (fenêtres, depth,
max_children en forme — comptage réel en F).
Vecteurs E1 (grant simple), E2 (chaîne profondeur 2), E3+ (tous les
fail-closed : sur-large, splice, fenêtre, kex mismatch, wildcard binding…).
**CLI :** `grant`, `delegate`, `verify`.
**Manuel :** ton cas d'usage : grant `read.circle#dir=projets/perso&tag=toto`
→ l'agent lit la section taguée, PAS la voisine non taguée ; `verify` avant/
après expiration.
**Done :** E1–E3 verts ; features étendues (grant + lecture déléguée).

### F — Gamma (spec 07)
Chaînage SHA-256, entrées (kinds), signatures owner/délégué, comptage
sous-arbre `authorized_via` (max_actions), `grant` compté (max_children),
heartbeat (§07.5), ancre de fraîcheur (§07.7). Pas encore : merge entries (→ I).
**CLI :** `action`, `heartbeat`, `log show|verify`.
**Manuel :** 3 actions avec `max_actions: 3` → la 4ᵉ rejetée ; owner silencieux
au-delà de every+grace → mandat heartbeat suspendu.
**Done :** F1–F3 verts ; features étendues (action comptée, budget épuisé).

### G — Révocation (spec 06) — après F : les révocations sont des entrées gamma
Échelle complète : cert (entrée gamma ancrée), rotation atomique + re-scellement
survivants + up-link wrap (§03.4 2bis), re-chiffrement, cascade, ré-adoption,
watchdog (verbe revoke sans clé), move-as-rotation (§02.9).
**CLI :** `revoke [--mode]`, `adopt`, `folder move`.
**Manuel :** révoquer l'agent de l'étape E → il ne lit plus rien de nouveau,
le survivant ne remarque rien, le détenteur de zone lit encore via l'up-link.
**Done :** G1–G4 verts (dont : rotation par non-autorisé → rejet) ; features étendues.

### H — Merkle (spec 02.10)
`H_leaf/H_node` domain-separated, hash de nœud (ligne ‖ header ‖ wraps ‖
enfants), racines par zone dans le manifest, preuves d'inclusion, sous-arbre
`dir=`, diff par descente.
**CLI :** `prove`, `edition diff`.
**Manuel :** `prove circle projets/perso/note1` → vérifier hors-ligne ;
modifier une section → seul son chemin de racine change dans le diff.
**Done :** H1–H3 verts (dont splice leaf/node → rejet) ; features étendues.

### I — Concurrence (spec 02.6, 07.6)
Merge déterministe d'éditions disjointes, fork same-node + résolution par plus
proche gestionnaire commun, entrées gamma `merge` (prevs), reconstruction
identique des racines Merkle par le mergeur et les vérificateurs.
**CLI :** `edition merge` (ou automatique dans publish).
**Manuel :** deux copies du bundle, deux écritures disjointes, merge → une
édition, tout vérifie.
**Done :** I1–I2 verts ; features étendues (deux agents concurrents).

### K — Intégration finale & packaging
Scénario K de la spec complet dans la suite de features (il l'est presque
déjà par accumulation), vecteurs de perf §09.3 en bench, image Docker (`FROM scratch`),
paquet npm `@aithos/core` (wasm-pack), conformance levels §09.4 documentés.
**Manuel :** dérouler le scénario K entier au CLI, chronométrer les cibles.
**Done :** tout vert, bench dans les cibles, image < 15 Mo, npm importable.

## Suivi

| Étape | Statut |
|---|---|
| 0 Conventions | **faite** — wire multibase, JCS, schéma vecteurs, harnais cucumber |
| A Genèse | **faite** — A1 + A2 verts, feature identité complète (9 scénarios) : genèse, clé de succession, DID doc, transition d'époque succession-only. Design amendé en cours de route : clé de contenu unique + politique de signature par zone (§02.11) |
| B Dérivation | **faite** — B2 vert (chaîne profonde + ancres tag), node_key(), 6 scénarios BDD (déterminisme, pas de portée latérale, rename sans re-clé, ancre locale ≠ racine), covers() par segments, CLI node-key |
| C Scellés | **faite** — C1/C2 verts (ECIES cross-checké Python au byte près), header I3 fail-closed, grant O(1) byte-identique, rotation + up-link wrap, 8 scénarios BDD, CLI header-seal/open. Spec §3.8 (construction normative), éphémère par ligne |
| D Bundle | **faite** — 8 scénarios e2e verts : chaîne d'éditions signée (manifest JCS #root, prev_hash, fail-closed tamper/chaîne), round-trip circle, rename sans re-clé, public lisible sans clé, self opaque vérifié en adversaire + reconstruction par descripteurs. FsStore + MemStore, CLI complet (init --dir, folder-add, section-add, zone-show, section-read, edition-publish/verify). Accroches prévues : pins plats → Merkle (H), gamma_ref (F) |
| E Mandats | **faite** — 11 scénarios verts : certificat pur (fenêtre à T injecté, kex vérifié pas cru), périmètre exact (sous-arbre, dir&tag fondateur, pas de latéral), multi-périmètres une clé (dont cross-branch), délégation atténuée + 3 fail-closed. mandate.rs (grammaire, covers(), verifier), grants.rs (grant = cert + lignes + vue tag + wraps, delegate offline). Vecteur E1 (JCS + signature cross-checkés Python). CLI grant/mandate-verify/section-read-agent |
| F Gamma | à faire |
| G Révocation | à faire |
| H Merkle | à faire |
| I Concurrence | à faire |
| K Intégration | à faire |
