# Prompt de reprise — scission protocole / service

Copier-coller le prompt ci-dessous dans une nouvelle tâche ouverte à la racine
`/Volumes/Math17/aithos/v2`.

---

Tu reprends le chantier Aithos d'assainissement de la frontière protocole/service
puis de scission du dépôt `aithos-core` en deux dépôts.

Le workspace contient plusieurs dépôts liés :

- `code/aithos-core` — le dépôt à assainir puis scinder ;
- `code/aithos-client` — dépôt séparé, déjà consommé par `path` dep et pinné par
  SHA dans la CI : **c'est le patron à copier** pour la dépendance inter-dépôts ;
- `code/aithos-sdk`, `code/aithos-client`, `code/aithos-sdk-example`.

## Sources de vérité à lire entièrement avant toute modification

1. `code/aithos-core/docs/CHANTIER-SPLIT-REPO-GATEWAY-SERVICE-2026-07-30.md` —
   le backlog canonique : décision, constat vérifié avec références de ligne,
   invariants, dix lots SPL-0 → SPL-9 avec critères de sortie.
2. `code/aithos-core/LICENSE` — la frontière cible est déjà écrite là, par chemin.
3. `code/aithos-core/rust/crates/aithos-gateway/src/lib.rs` — la règle de couche
   telle qu'énoncée, à comparer à la règle réellement appliquée.
4. `code/aithos-core/spec/02-content-tree.md` et `spec/08-connectors.md` — la
   norme du namespace `x/<id>`, cible de la migration du lot SPL-2.

Lis le document 1 jusqu'à EOF. Ne résume pas sa lecture à partir de ce prompt :
utilise réellement son contenu comme backlog, lot par lot, dans l'ordre.

## Objectif final

Deux dépôts, dont la frontière est celle que la licence décrit déjà :

- `aithos-core` : `aithos-core`, `aithos-bundle`, `aithos-cli`, `aithos-wasm`,
  le crate de cérémonies propriétaire créé au lot SPL-4, `spec/`, `features/`,
  les vecteurs protocolaires ;
- `aithos-service` : `aithos-gateway` + `aithos-provider`, leurs features, leurs
  vecteurs `p*`.

Et, en cours de route, trois dettes payées : la grammaire de bundle libérée du nom
`gateway`, les ~1 685 lignes de cérémonies propriétaire sorties du crate gateway,
la surface CLI unifiée.

## Discipline de travail obligatoire

- **Travaille dans un worktree dédié.** Jamais sur la branche par défaut. Ne
  committe ni ne pousse sans demande explicite.
- **Un lot = une branche, une fonction déplacée = un commit.** Pour le lot SPL-4
  en particulier : déplacer, génériquiser, compiler, rejouer la baseline,
  committer. Jamais deux fonctions dans le même commit.
- **Ne commence aucun lot sans avoir rejoué `scripts/split-baseline.sh`** (créé
  au lot SPL-0) et constaté qu'il est vert sur l'arbre de départ.
- **Le code est arbitre.** Les références de ligne du document 1 sont datées du
  2026-07-30 : revérifie-les au début de chaque lot, et si elles ont bougé, mets à
  jour le document avant de coder.
- **Aucun faux vert.** Un test qui ne prouve pas ce que son nom annonce est un
  bug du chantier. Un scénario `@wip` reste `@wip` : ce chantier ne livre aucun
  comportement nouveau.
- **Les compteurs de tests ne baissent jamais.** Toute baisse est un blocage, pas
  un détail à expliquer.
- Si un critère de sortie ne peut pas être atteint, **arrête le lot et remonte la
  décision** plutôt que d'assouplir le critère.

## Point de départ vérifié le 2026-07-30

- Six crates dans `rust/Cargo.toml` ; `aithos-gateway` = 38 000 lignes de `src` +
  29 144 de `tests` ; `aithos-provider` = 21 454 ; le protocole + CLI + WASM =
  65 699.
- **Le bloc `owner_*` de `core_bridge.rs` (l. 4785 → 6470) n'a aucun appelant dans
  `impl Bridge` ni `impl Runner`** — seulement `src/main.rs` et les tests. C'est le
  fait qui rend le lot SPL-4 sûr. Exception : `manifest_tool_pin` (l. 6170), utilisée
  par `hub.rs`, `connectors.rs`, `compiled_extensions.rs`, qui reste.
- `owner_read_briefing` (l. 5481) est du code mort : aucun appelant, nulle part.
- `validate_store_key` (`aithos-bundle/src/lib.rs:156-157`) énumère
  `gateway/state.json` et `gateway/keys.json`. Le namespace d'accueil `x/` existe
  déjà (`connector_object_accepted`, l. 115) et **`x/gateway/state.json` est déjà
  une clé valide aujourd'hui** — le lot SPL-2 retire une arme nominative, il n'en
  ajoute pas. `gateway/keys.json` n'est écrit par personne et trois tests
  asservissent son absence.
- Le verbe `act.x.gateway.*` est déjà le périmètre de la gateway
  (`core_bridge.rs:460`, `ethos_catalog.rs:33-39`) : la migration aligne l'objet
  sur le nœud dont le verbe le gouverne.
- Les features sont propres : trois corpus séparés par couche, deux saignements
  mineurs seulement (détail au §2.5 du document 1).
- `aithos-gateway/assets/ceremony/aithos_wasm_bg.wasm` est un build de
  `aithos-wasm` figé au 22/07 dont la dérive n'est **pas** détectée aujourd'hui.

## Ordre d'exécution attendu

`SPL-0` → `SPL-1` → `SPL-2` → `SPL-3` → `SPL-4` → `SPL-5` → `SPL-6` → `SPL-7`
→ `SPL-8` → `SPL-9`, sans saut.

**SPL-0 à SPL-5 ne touchent pas à Git.** À leur terme, l'essentiel du bénéfice
architectural est acquis et la scission n'est plus qu'un déplacement mécanique.
Un arrêt du chantier après SPL-5 doit laisser le dépôt dans un état strictement
meilleur qu'au départ : c'est une contrainte de conception du plan, respecte-la.

Deux décisions restent à prendre **au début** de leur lot, et à consigner dans le
document 1 :

1. **SPL-2** : `act.x.gateway.*` couvre-t-il réellement `x/gateway/state.json` sur
   le wire provider (`pathmap.rs`, couverture `act.x.<id>.*` → GET/PUT
   `x/<id>/**`) ? Si non, le lot s'arrête et la tension entre
   `docs/REDLINE-A1-DRAFT2-PROPOSITION-GATE5-2026-07-20.md:134` (« pas une route
   wire ») et `store_adapter.rs:321` (lecture depuis le store répliqué) remonte
   en arbitrage.
2. **SPL-4** : les cérémonies propriétaire génériques vont-elles dans
   `aithos-bundle` ou dans un crate `aithos-owner` neuf ? Le document recommande
   le crate dédié et explique pourquoi.

## Commandes de baseline utiles

```sh
cd code/aithos-core
cargo fmt --all --manifest-path rust/Cargo.toml -- --check
cargo clippy --workspace --all-targets --manifest-path rust/Cargo.toml -- -D warnings
cargo test --workspace --manifest-path rust/Cargo.toml
cargo check -p aithos-wasm --target wasm32-unknown-unknown --manifest-path rust/Cargo.toml
```

Vérifications de frontière, à rejouer à chaque fin de lot :

```sh
# aucun consommateur nommé dans le protocole (gate du lot SPL-2)
grep -rn 'gateway/state.json\|gateway/keys.json' rust/crates/aithos-bundle rust/crates/aithos-core

# qui importe le protocole dans la gateway (règle de couche du lib.rs)
grep -rln 'aithos_core\|aithos_bundle\|aithos_client' rust/crates/aithos-gateway/src

# cérémonies propriétaire restantes dans le crate gateway (gate du lot SPL-4)
grep -c 'pub fn owner_' rust/crates/aithos-gateway/src/core_bridge.rs

# chemins relatifs vers les vecteurs (gate du lot SPL-1)
grep -rn '\.\./\.\./\.\./vectors' rust/crates/
```

## Condition d'arrêt

Le chantier est terminé quand les gates du §5 du document 1 sont toutes vertes,
**et** qu'une démo de bout en bout a été rejouée après scission. Tant que ce
dernier point n'est pas fait, le chantier est en cours, quel que soit l'état des
tests unitaires.

Ne supprime les crates service de `aithos-core` qu'en **dernier commit** du lot
SPL-8, une fois les deux CI vertes, et dans un commit isolé et revertible.
