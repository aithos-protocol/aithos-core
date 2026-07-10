# Aithos Core — Handoff (reprise en contexte neuf)

**But.** Reprendre l'implémentation de référence d'aithos-core sans rien reperdre.
Résume où on en est, comment on travaille, et la prochaine étape exacte.

Dernier commit : `cd6fc36` (step E complete). Repo : `code/aithos-core/`.

---

## 1. Ce qu'est aithos-core

Couche de confiance pour agents IA, **enforceable depuis les fichiers seuls, sans
serveur de confiance** : identité, mandats scopés, délégation récursive, révocation
scopée, log d'actions inviolable. Primitives : BLAKE3 (dérivation), XChaCha20-Poly1305
(AEAD), Ed25519 (signatures), X25519+HKDF-SHA256 (scellés ECIES). La spec normative
vit dans `spec/00..10` — **source de vérité**.

Profil cible : **absentee owner** — l'owner émet un mandat large puis ne repasse
presque jamais ; toute la maintenance est récursive (les gestionnaires délégués), pas
owner-dépendante.

## 2. Méthode de travail (à respecter à la lettre)

Rituel par étape du plan (`docs/EXECUTION-PLAN.md`) :

1. **Feature d'abord (BDD).** On co-écrit le `.feature` Gherkin (anglais) AVANT le
   code, scénarios taggés `@wip` (le runner les saute → suite verte). C'est le contrat.
2. **Vecteurs d'abord (TDD).** Valeurs attendues générées **indépendamment** en Python
   (`blake3` + `PyNaCl` + `base58`) quand c'est de la crypto — jamais d'auto-certification.
3. **Impl** jusqu'à vecteurs + scénarios verts ; on dé-tagge `@wip` au fur et à mesure.
4. **Checkpoint manuel CLI** (non bloquant) déroulé pour "sentir" le produit.
5. **Commit** en anglais, clair. Validation utilisateur → étape suivante.

Règles gravées :
- **Pureté du core** : `aithos-core` ne fait aucune I/O, aucune horloge, aucun RNG.
  Temps `T`, aléa et stockage sont **injectés**. C'est ce qui rend tout déterministe,
  rejouable contre `vectors/`, et compilable en WASM.
- **DDD-lite** : chaque type de code porte le nom exact de son concept spec (Mandate,
  PerimeterEntry, NodePath, Header, Edition…), frontières = chapitres.
- **Fail-closed** : chaque rejet = un variant nommé de `error.rs`.
- **Décisions wire figées (étape 0)** : multibase base58btc (`z6Mk` ed25519 /
  `z6LS` x25519), JCS RFC 8785 pour tout ce qui est signé/haché, AAD `§00.3`.

## 3. Décisions de design importantes prises en cours de route

- **Une seule clé de contenu owner** (`content_sign`), PAS trois sphères. L'audience
  vit dans le **payload signé** (`{zone, path, sid, body_hash}`), jamais dans la clé.
  public = signé au clair ; circle = signé sous scellé ; **self = jamais signé**
  (déniable par défaut ; divulgation sélective = mécanisme officiel). Spec §02.11.
- **Décision B** : le mandat de tête porte un dead-man heartbeat par défaut (30 j),
  signé par une clé de liveness, jamais dans le périmètre de l'agent de tête.
- **Clé de succession** : froide, NON dérivée de S, seule autorité pour déclarer une
  nouvelle clé maîtresse (transition d'époque). Inventaire owner : root, content, kex,
  succession.
- **Arbre profond** : zones = dossiers racines, récursion sans limite ; sids stables
  = labels de dérivation (renommer ne re-chiffre jamais ; déplacer = rotation).
- **Une clé, N périmètres** : un agent = 1 keypair Ed25519. Un grant scelle une COPIE
  de la DK vers sa clé (une ligne de header) ; N périmètres = N lignes, une seule clé.
- **Merkle** (spec §02.10) : conçu, PAS encore implémenté (étape H). Le manifest
  épingle les fichiers à plat en attendant — point d'accroche prévu.

## 4. État du code (6 étapes closes sur 11)

Workspace cargo 4 crates : `aithos-core` (pur), `aithos-bundle` (I/O, Store),
`aithos-cli` (binaire), `aithos-wasm` (bindings).

| Étape | Statut | Livré |
|---|---|---|
| 0 Conventions | ✅ | wire.rs (multibase), jcs.rs, cucumber harness, vectors/README |
| A Identité | ✅ | keys.rs (genesis, succession, ed2x), did.rs (DID doc + epoch transition) — A1, A2 |
| B Dérivation | ✅ | derive.rs::node_key (1 dérivation/segment), path.rs (NodePath, covers) — B2 |
| C Scellés | ✅ | seal.rs (ECIES + wrap + blob AEAD), header.rs (I3, grant, rotate, up-link) — C1, C2 |
| D Bundle | ✅ | bundle.rs (3 zones, éditions signées, self opaque), manifest.rs, entropy.rs, FsStore |
| E Mandats | ✅ | mandate.rs (grammaire, covers, verifier à T), grants.rs (grant+delegate) — E1 |
| F Gamma | ⏳ **prochaine** | — |
| G Révocation | ⬜ | (après F : révocations = entrées gamma) |
| H Merkle | ⬜ | racines d'état, preuves |
| I Concurrence | ⬜ | merge disjoint, fork |
| K Intégration | ⬜ | scénario K complet, Docker, npm |

**Tests actuels : 42 scénarios / 147 steps cucumber + vecteurs A1/A2/B2/C1/E1, tous
verts ; clippy clean.**

## 5. Comment builder / tester

Rust installé hors du repo (voir env). Depuis `code/aithos-core/` :

```
cargo test  --workspace --manifest-path rust/Cargo.toml   # 42 scénarios + vecteurs
cargo clippy --workspace --manifest-path rust/Cargo.toml -- -D warnings
```

CLI (verbes existants) : `init --dir`, `folder-add`, `section-add`, `zone-show`,
`section-read` (public sans clé), `edition-publish`, `edition-verify`, `grant`,
`mandate-verify`, `section-read-agent`, + debug `node-key`, `header-seal/open`.

Note environnement sandbox : `rustup`/`cargo` sous `/tmp/cargo`, `/tmp/rustup` ;
`CARGO_TARGET_DIR=/tmp/target CARGO_INCREMENTAL=0` (disque limité, purger
`/tmp/target/debug/incremental` si "No space left"). Le repo git a parfois des
`.git/*.lock` à supprimer avant commit (permissions sandbox).

## 6. Prochaine étape : F — Gamma (spec §07)

Le journal chaîné, substrat d'enforcement des contraintes. À implémenter :
chaînage SHA-256 des entrées, kinds (`section.add/modify/delete`, `action`, `grant`,
`revoke`, `rotate`, `heartbeat`, `merge`), signatures owner (`content_sign`) vs
délégué (keypair + `authorized_via`), **comptage sous-arbre** `max_actions` via
`authorized_via`, `max_children` via entrées `grant`, heartbeat (§07.5), **ancre de
fraîcheur** anti-antidatage (§07.7). Pas encore : merge entries concurrentes (→ I).

Rituel : co-écrire `features/f-gamma.feature` d'abord (chaîne inviolable, budget
`max_actions` épuisé à la N+1, heartbeat suspend au-delà de every+grace, entrée
antidatée hors ancre rejetée), vecteur F Python, puis impl `gamma.rs` dans le core +
verbes CLI `action`/`heartbeat`/`log show|verify`.

**Dépendance importante** : G (révocation) s'appuie sur F — c'est pour ça que le plan
a mis gamma AVANT révocation (les révocations sont des entrées gamma, §06.5).

## 7. Points ouverts / dette assumée (non bloquants)

- Manifest à pins plats jusqu'à H (Merkle).
- `section_add`/`grant`/`delegate` ont beaucoup d'arguments (`#[allow(too_many_arguments)]`) —
  un struct de params est prévu à F.
- Édition : pas encore de merge/fork (étape I).
- Artefact de récupération combiné (S ‖ succession en une mnémonique) : couche de
  présentation pure, ajoutable bien plus tard, zéro impact protocole.
