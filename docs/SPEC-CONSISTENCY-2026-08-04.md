# Passe de cohérence de la spécification — 2026-08-04

> **Rôle** : cohérence interne de `spec/` et satisfiabilité par le code.
> Ce document ne juge aucune feature, n'audite aucun scénario Gherkin, ne corrige rien.
> **Aucun fichier de `spec/` n'a été modifié.** Aucune commande `cargo`, `git`, aucun
> test, aucun build n'a été lancé **par la passe elle-même** ; quatre gates ont été
> exécutés par l'orchestrateur le 2026-08-04, après la levée d'embargo, pour les
> seuls SC-05 et SC-12 (`ev-cb4ff302`, `ev-fafd51d8`, `ev-63e018d1`,
> `ev-b8cee044`). Ils établissent une ligne de base verte à `223924e` ; ils ne
> démontrent aucun défaut. Les onze autres constats restent des conclusions de
> lecture pure.
>
> Périmètre lu : `spec/` intégralement (4 348 lignes, 11 fichiers, 79 énoncés
> `MUST`/`SHALL`), `rust/crates/**/src/**` (28 603 lignes de source hors tests),
> `vectors/`, `rust/crates/**/tests/**` en lecture ciblée.
>
> **Tous les arbitrages de ce document sont `PROPOSÉ — NON IMPLÉMENTÉ`.** Leur
> faisabilité n'a pas été vérifiée et ce document ne prétend pas l'avoir fait :
> c'est un cycle de correction ultérieur qui la vérifiera. C'est rappelé
> explicitement sur chaque constat.

---

## Synthèse

**13 constats.**

### Par famille

| Famille | Nombre | Identifiants |
|---|---:|---|
| 1 — Contradiction (deux énoncés normatifs incompatibles) | 7 | SC-01, SC-02, SC-04, SC-05, SC-10, SC-12, SC-13 |
| 2 — Lettre morte (énoncé que rien n'implémente) | 3 | SC-03, SC-07, SC-09 |
| 3 — Inatteignable (aucun chemin du code ne permet la satisfaction) | 3 | SC-06, SC-08, SC-11 |

### Par classe

| Classe | Nombre | Identifiants |
|---|---:|---|
| `TEXTUELLE` | 3 | SC-02, SC-10, SC-13 |
| `LES DEUX IMPLÉMENTÉS` | 3 | SC-01, SC-04, SC-12 |
| `AUCUN` | 4 | SC-03, SC-05 (moitié code), SC-07, SC-09 |
| `INATTEIGNABLE` | 3 | SC-06, SC-08, SC-11 |

SC-05 compte deux fois : sa moitié spec-contre-spec est `TEXTUELLE` sur
`spec/04-mandates.md:238`, sa moitié code est `AUCUN`. Le total reste de treize
constats. **Les deux classes retenues du 2026-08-04T07:40Z au
2026-08-04T13:00Z sont publiées** : celle de SC-12 l'était déjà, celle de SC-05
l'est depuis la levée — elle avait été retenue parce qu'elle se déduit
directement du bord code, et qu'elle en donne le sens.

### Constats touchant un invariant nommé

| Constat | Invariant | Classe |
|---|---|---|
| SC-03 | I5 | `AUCUN` |
| SC-04 | I5 | `LES DEUX IMPLÉMENTÉS` |
| SC-06 | I3 | `INATTEIGNABLE` |
| SC-07 | I4 | `AUCUN` |
| SC-08 | I4 | `INATTEIGNABLE` |
| SC-11 | I3 | `INATTEIGNABLE` |
| SC-12 | I4 | `LES DEUX IMPLÉMENTÉS` |

### Ce que la passe **n'a pas** trouvé

Les cinq invariants `I1`–`I5` ont été rassemblés dans leurs 35 occurrences
recensées (`grep -rn "\bI[1-5]\b" spec/`, 10 fichiers). **Aucune dérive de
formulation d'un invariant contre un autre énoncé du même invariant n'a été
trouvée dans `spec/`** : les réénoncés d'I1, I2 et I5 sont réductifs mais non
contradictoires, et les réénoncés d'I3 (§0.2:35-40, §01.1:23, §03.1:45-50,
§05.5:88-92, §08.2:198, §09.4:101, §10.1:19) et d'I4 (§0.2:41-45, §05.5:80-81,
§06.4:76-84, §06.5:96, §10.1:15, §10.5:49) sont mutuellement cohérents mot à mot.
La dérive attendue au lieu naturel n'y est pas. Les six divergences qui touchent
I3/I4/I5 ci-dessus proviennent **toutes** du bord code, ou d'une clause satellite
(§03.4, §05.5, §02.2, §07.9.2), jamais du texte de l'invariant lui-même.

### Blocage — deux rétentions, levées le 2026-08-04

**Les deux rétentions de cette passe sont levées et les deux constats sont
publiés en entier.** Le 2026-08-04T07:40Z, SC-12 (en entier) et la moitié code de
SC-05 ont été retenus sous la « condition 9 » de la barrière de divulgation,
telle que `features/.agents/orchestrator/BLOCKED.md` la nomme : dépôt public,
faiblesse exploitable, aucun correctif disponible. Le
2026-08-04T13:00Z, le propriétaire (Mathieu Colla) a tranché : publication
intégrale des deux, au motif que le correcteur doit pouvoir citer ce qu'il
répare, et que rien n'étant déployé, une divulgation ne coûte rien à personne
tandis qu'une rétention coûte une correction. La décision est consignée dans
`features/.agents/orchestrator/BLOCKED.md` § « Résolues ».

**Les deux constats sont re-dérivés, pas restitués.** Leur texte complet vivait
hors du dépôt, dans `/root/work/EMBARGO-SC-12.md` et `/root/work/EMBARGO-SC-05.md`.
Ces deux fichiers n'existaient plus au moment de la levée : l'effacement
silencieux du clone local les a détruits. Les énoncés publiés ci-dessous ont donc
été **re-dérivés depuis `spec/` et depuis le code à `223924e`**. Chacun porte sa
note de levée d'embargo, et chacun signale les points du dossier survivant que la
re-dérivation contredit — il y en a, et c'est précisément l'argument pour
re-dériver plutôt que restituer : une re-dérivation se vérifie ligne à ligne.

**Ce que la re-dérivation a révélé, et qui vaut mieux que la décision elle-même.**
`SC-12` était retenu, entre autres, parce que le durcir invaliderait
rétroactivement des entrées publiées, et que §0.4 n'autorisait qu'un seul
durcissement rétroactif dans la série. Cette phrase de `spec/00-overview.md` avait
été **supprimée par l'orchestrateur lui-même** — commit `c8557f4`, appliquant la
décision du propriétaire sur la condition de blocage 1 — vingt-quatre minutes
après le commit de cette passe, sans que personne voie qu'un constat d'un autre
document reposait dessus. Le constat est resté sous embargo cinq heures, défendu
par une phrase inexistante. Personne ne l'a vu jusqu'à ce qu'il soit re-dérivé
depuis les sources plutôt que restitué de mémoire ; si le fichier hors dépôt avait
survécu, il aurait été republié tel quel, argument mort compris. Le détail est en
`SC-12`, note de levée d'embargo.

Deux conséquences de process, portées hors de ce document. D'abord, une barrière
de divulgation hors dépôt est une rétention **sans durabilité** : celle-ci a
détruit deux énoncés sur trois à sa première levée — `QUEUE.yaml`,
`disclosure-barrier-durability`. Ensuite, une correction dans un document peut
retirer silencieusement la prémisse porteuse d'un constat retenu dans un autre,
et une rétention hors dépôt soustrait précisément ce constat aux relectures qui
l'attraperaient.

Corollaire sur la lisibilité, désormais résolu : trois autres endroits (SC-07,
SC-13, et la note de méthode) avaient vu une référence de code retirée ou
généralisée parce qu'elle rouvrait par une autre porte l'un des sites retenus.
**Les deux pointeurs nommés — la citation `check_revoke_authority` de SC-07 et
l'intervalle de lignes `mandate.rs` de SC-13 — sont restaurés**, chacun signalé
sur place avec la confirmation qu'il n'avait pas d'autre motif de retrait. La
note de méthode est mise à jour au même titre.

---

## SC-01 — Emplacement canonique du journal Gamma

**Famille 1 — Contradiction. Classe : `LES DEUX IMPLÉMENTÉS`.**

### (1) Les deux côtés, verbatim

`spec/02-content-tree.md:59-73` (bloc « 2.3 Bundle layout ») :

```
manifest.json              signed, linear-chained (§2.6)
did.json                   §01.4
…
gamma/gamma.jsonl          §07
```

et `spec/02-content-tree.md:88-99`, qui rend cette table opposable :

> Store keys are likewise relative and confined, but obey the exact canonical
> layout of §2.3 (whose fixed filenames and extensions are not human names), not
> the §2.2 name grammar.

`spec/07-gamma.md:9-14` :

> `gamma/<YYYY-MM>.jsonl` — one JSON entry per line, SHA-256 hash-chained,
> segmented by UTC month of `at` (a month with no entries has no file; `prev`
> crosses segment boundaries transparently). Segmentation buys date-range access
> in O(segments touched) and leaves room for per-segment keys later; the chain,
> not the file layout, is the truth. The manifest pins every segment's hash plus
> `gamma_head` (§02.7).

`spec/01-identity-and-keys.md:96` fige le premier des deux dans un document signé :

> ```jsonc
>   "revocations": "gamma/gamma.jsonl",        // revocation-state pointer (§06.5)
> ```

### (2) Ce que le code fait de chaque côté

Les **deux** sont implémentés.

- Côté §07.1 (segmenté) — c'est le chemin d'écriture réel.
  `rust/crates/aithos-bundle/src/log.rs:29` :
  ```rust
  Ok(format!("gamma/{}.jsonl", &at[..7]))
  ```
  et `rust/crates/aithos-bundle/src/log.rs:136` dépouille le préfixe `gamma/`
  pour retrouver le segment. `rust/crates/aithos-bundle/src/remote.rs:16`
  documente `put("gamma/<YYYY-MM>.jsonl")` comme le contrat d'append distant.
- Côté §02.3 (fichier unique) — c'est une clé de store légale et le pointeur
  DID publié. `rust/crates/aithos-bundle/src/lib.rs:155` liste
  `"gamma/gamma.jsonl"` dans la grammaire fermée `validate_store_key`, et
  `rust/crates/aithos-bundle/src/lib.rs:188` la ré-accepte à côté de la forme
  `YYYY-MM`. `rust/crates/aithos-cli/src/cmd/init.rs:70` écrit
  `"gamma/gamma.jsonl"` dans le champ `revocations` du document DID à
  l'initialisation, et `vectors/a2-did.json:9` gèle cette valeur dans un vecteur.

Conséquence : le pointeur de révocation signé de §06.5 désigne un objet que le
chemin d'append n'écrit jamais. Aucun code ne résout `revocations` vers les
segments réels ; la grammaire de store accepte les deux formes sans les relier.

### (3) Classe

`LES DEUX IMPLÉMENTÉS`. Les deux comportements coexistent : la forme segmentée
comme chemin d'écriture, la forme unique comme clé légale et comme valeur signée
dans `did.json`. Arbitrer veut dire en supprimer un, avec un coût réel.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Aligner §02.3 sur §07.1 (`gamma/<YYYY-MM>.jsonl`) et redéfinir `revocations`
comme un **préfixe** de segment plutôt qu'un fichier, §06.5 le désignant déjà
comme un « revocation-state pointer » et non comme un agrégat.

- Coût estimé : moyen. Une ligne de `spec/02-content-tree.md`, une de
  `spec/01-identity-and-keys.md`, la suppression de deux branches de
  `validate_store_key`, et la réécriture de `cmd/init.rs:70`.
- Ce que cela casserait : `vectors/a2-did.json` (octets signés du document DID —
  la signature couvre `revocations`), donc **tout document DID déjà publié**.
  §01.4 fait porter la signature sur le JCS complet ; changer `revocations`
  change les octets signés. Une migration serait une transition d'époque
  d'identité (§10.4) ou une réémission de `did.json` sous `#root`.
- **La faisabilité de cet arbitrage n'a pas été vérifiée par ce rôle.**

---

## SC-02 — Inventaire des purposes AAD de §00.3

**Famille 1 — Contradiction. Classe : `TEXTUELLE`.**

### (1) Les deux côtés, verbatim

`spec/00-overview.md:62-65` :

> AAD convention, NUL-separated after the purpose label
> `"aithos-core/v1/<purpose>"`: `subject_did ‖ node_path ‖ key_version` for content
> purposes (`blob`, `tagwrap`, `vault`, `gamma-payload`), `subject_did ‖ header_path ‖
> key_version` for `header-line`. Purposes never overlap.

`spec/07-gamma.md:120-127` :

> **Sealed bodies (content mutations).** For every `section.*` entry on a keyed
> zone (`circle`, `self` — `public` has no zone key and its mutations stay clear,
> target and payload at the top level like structural kinds), the body
> `{target, payload}` is AEAD under the **target node's content key** (derivation
> purpose `gamma-body`): the log reveals *that* someone acted at some time under
> some mandate, but *what was touched and what changed* is readable only by those
> who can read the node itself.

`spec/03-headers.md:32` (le même AAD, nommé autrement) :

> AAD purpose `header-line`, bound to `subject_did ‖ node ‖ key_version`.

Trois divergences dans une seule phrase de §00.3 : le purpose gamma s'y appelle
`gamma-payload` et ailleurs `gamma-body` ; un purpose `vault` y est déclaré ; et
la liaison de `header-line` y est `header_path` alors que §03.1 et §03.8 disent
`node`.

### (2) Ce que le code fait de chaque côté

Un seul côté est implémenté — celui de §07.3 et §03.8.

- `rust/crates/aithos-core/src/gamma.rs:21` :
  ```rust
  pub const PURPOSE_GAMMA_BODY: &[u8] = b"aithos-core/v1/gamma-body";
  ```
  `gamma-payload` n'apparaît **nulle part** dans `rust/`, `vectors/` ni `spec/`
  hors de cette unique ligne §00.3 (recherche `grep -rn "gamma-payload" rust/
  spec/ vectors/ docs/`, couche code **et** couche corpus de données).
- Aucun purpose `vault` n'existe. Le vault réutilise l'AAD `blob` :
  `rust/crates/aithos-bundle/src/vault.rs:146-148` et `:161-163` construisent
  l'AAD via le même constructeur que les blobs ordinaires, sur le node
  `/x/<connector>`. La seule chaîne contenant `vault` est
  `rust/crates/aithos-bundle/src/vault.rs:70`,
  `b"aithos-core/v1/vault-config-record\0"`, qui est un préimage de commitment
  de clé d'enregistrement — pas un purpose AEAD, pas dans la liste de §00.3.
- L'AAD `header-line` est bâtie sur le node : `rust/crates/aithos-core/src/header.rs:261`
  appelle `line_aad(subject_did, &self.node, version)`.

### (3) Classe

`TEXTUELLE`. Le côté §00.3 est lettre morte intégrale : ni `gamma-payload`, ni un
purpose `vault`, ni un `header_path` distinct du node n'existent dans le code ou
dans les vecteurs. Correction bon marché — mais elle porte sur des chaînes de
séparation de domaine, donc son énoncé actuel est activement trompeur pour une
seconde implémentation qui lirait §00.3 en premier (c'est l'ordre de lecture
prescrit par §00.6).

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Réécrire la phrase §00.3 en : purposes de contenu `blob`, `tagwrap`,
`gamma-body` ; `header-line` lié à `subject_did ‖ node ‖ key_version`. Supprimer
`vault` de la liste et ajouter une phrase renvoyant §08.2 au purpose `blob`.

- Coût estimé : faible. Une phrase de `spec/00-overview.md`. Aucun octet signé.
- Ce que cela casserait : rien au niveau du wire. Cela invalide en revanche la
  ligne 15 et la ligne 226 de
  `docs/research/topology-2026-07-28-unverified/lot-A-00-01-03-10.md`, qui
  recensent la divergence comme ouverte.
- **La faisabilité de cet arbitrage n'a pas été vérifiée par ce rôle.**

---

## SC-03 — Aucun `kind` Gamma pour une mutation structurelle ou vault-config

**Famille 2 — Lettre morte. Classe : `AUCUN`. Invariant touché : I5.**

### (1) L'énoncé, verbatim

`spec/00-overview.md:46-49` (I5) :

> 5. **I5 — No silent actions.** Every mutation and every connector action performed
>    under a mandate MUST be recorded as a gamma entry naming the mandate. An action
>    without its entry is invalid; verifiers treat the entry count as the mandate's
>    consumption meter.

`spec/07-gamma.md:326-338` — le registre est **fermé**, et sa clôture est
fail-closed :

> ### 7.9.2 Kind registry (normative)
>
> Kinds are a closed registry — an unregistered kind fails the entry (fail-
> closed). Naming: `<domain>.<verb>`, lowercase.
>
> | Kind | Class | Payload |
> |---|---|---|
> | `section.add/modify/delete/redact` | `ethos.write` | sealed body (keyed zones) |
> | `ethos.read` | `ethos.read` | sealed body naming the section read |
> | `action` | `act` | clear: action, args_hash, budget_ref?, tokens?, receipt?, checks?[] (§04.12) (+ sealed args body, §7.9.3) |
> | `inference` | `act` | clear counters (§7.9.1) |
> | `grant` / `revoke` / `rotate` / `merge` | structural | clear ids/versions |
> | `heartbeat` | liveness | clear `{seq}` |

`spec/04-mandates.md:792-799` définit pourtant une famille de mutations
structurelles distincte de la famille section :

> For the structural family, `verb` is exactly `create`, `rename`, `delete`, or
> `move`. `node_kind` uses the existing `folder` or `section` literals. `create` and
> `delete` admit `folder` only; section creation and deletion use the Ethos family.

et `spec/04-mandates.md:1818-1819` :

> The config-read evidence encoding is WIP; config mutations are always journalized
> independently of `log_reads`.

Le registre fermé de §07.9.2 ne contient **aucun** kind pour `domain:"structure"`
ni pour `domain:"vault-config"` (recherche exhaustive de la table §07.9.2 et de
la liste §07.1:25-27, couche spécification).

### (2) Ce que le code fait de chaque côté

Le registre fermé est implémenté **à la lettre**, avec exactement douze membres :
`rust/crates/aithos-core/src/gamma.rs:26-39` (`enum Kind`) et `:42-58`
(`Kind::parse`, `other => Err(Error::InvalidGammaEntry(...))`). Aucun kind
structurel ni vault-config.

Le côté §04.5.1 est implémenté **par surcharge des kinds section**, avec un
discriminant de payload que la spécification ne définit nulle part :

- `rust/crates/aithos-bundle/src/structure.rs:889` — un déplacement de dossier
  `circle` journalise `aithos_core::gamma::Kind::SectionModify` avec le payload
  `{"destination": …, "structural": "folder.move"}` ;
- `rust/crates/aithos-bundle/src/structure.rs:960` — idem pour `public` ;
- `rust/crates/aithos-bundle/src/structure.rs:1080-1085` — un changement de
  métadonnées journalise `Kind::SectionModify` avec `"structural": "section.metadata"` ;
- `rust/crates/aithos-bundle/src/vault.rs:276`, `:297`, `:318` — un
  create/edit/delete de config vault journalise respectivement
  `Kind::SectionAdd`, `Kind::SectionModify`, `Kind::SectionDelete`, sur un node
  `/x/<connector>` qui n'est pas une zone.

La clé `payload.structural` n'existe dans aucune section de `spec/` (recherche
`grep -rn "structural" spec/` : §07.3:135 et §07.9.2:337 emploient « structural »
comme **classe de requête** couvrant `grant`/`revoke`/`rotate`/`merge`, jamais
comme membre de payload — couche spécification).

### (3) Classe

`AUCUN`. La spécification décrit un comportement — journaliser une mutation de
dossier et une mutation de config vault sous I5 — absent des deux côtés : le
registre fermé ne lui offre pas de kind, et §04.5.1 ne dit pas quel kind employer.
Le code comble le trou par une convention non spécifiée. Une seconde
implémentation, écrite depuis `spec/` seule, produirait un discriminant différent
ou aucun ; les deux journaux seraient valides au registre et mutuellement
inintelligibles. Le classement `AUCUN` plutôt que `LES DEUX IMPLÉMENTÉS` est
délibéré : ce n'est pas qu'il y a deux comportements dans le code, c'est qu'un des
deux côtés n'a pas d'énoncé dans la spec.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Ouvrir le registre §07.9.2 avec deux familles nommées — par exemple
`folder.add/rename/move/delete` et `config.add/edit/delete` — dans une nouvelle
version `v` d'entrée Gamma, §07.1:42-47 prévoyant déjà que « the `v` field exists
so that transition is a version bump, not a fork ».

- Coût estimé : lourd. Nouveau `v`, nouvelle table de présence `operation_ref`
  (§07.1.1:72-79), nouvelle classe de requête, migration des trie de comptes H2
  (§07.10) dont les feuilles hachent le JCS exact de chaque ligne.
- Ce que cela casserait : toutes les entrées `section.modify` déjà écrites pour
  des mutations de dossier restent valides mais deviennent des reliquats
  historiques ininterprétables sous la nouvelle grille. Les racines de segment
  §07.10 hachent le JCS exact de chaque ligne, donc **aucune** réécriture n'est
  possible : la cohabitation est obligatoire. `vectors/cb2-operation-facts-structural.json`
  et `vectors/cb2-bundle-structure-vault.json` seraient à régénérer.
- Variante bon marché, à arbitrer contre la précédente : ne rien ouvrir, et
  spécifier `payload.structural` tel que le code l'écrit déjà — coût faible,
  mais grave un discriminant de payload dans des octets signés sans lui donner de
  table fermée, ce qui est exactement ce que §04.1.1:63-64 interdit ailleurs
  (« an emitter MUST NOT guess its bytes »).
- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle.**

---

## SC-04 — `ethos.read` émis hors de tout mandat `log_reads`, en payload clair

**Famille 1 — Contradiction. Classe : `LES DEUX IMPLÉMENTÉS`. Invariant touché : I5.**

### (1) Les deux côtés, verbatim

`spec/07-gamma.md:340-345` :

> **Classes** are query-level groupings: filtering on `kind=ethos.write` matches
> every `section.*` entry — wire kinds do not change (frozen vectors stay
> frozen). `ethos.read` entries exist only under a `log_reads` mandate (§04.4):
> reading is not journalized by default (I5 logs *acts*, not looks), and physics
> cannot force a reader's pen — the flag makes read-logging a contractual duty,
> checkable on presentations, honest about silent reads.

`spec/07-gamma.md:334` (registre §07.9.2, ligne `ethos.read`) :

> | `ethos.read` | `ethos.read` | sealed body naming the section read |

`spec/04-mandates.md:1805` (matrice §04.13) :

> | `log_reads` | P† | — | — | P-W† | — | — | — | — |

avec `spec/04-mandates.md:1776-1777` pour le suffixe `W` :

> suffix **W** = semantic applicability is validated and fixed, but its public
> encoding/proof is reserved for CB2 and cannot yet yield Allow;

Donc, en colonne `Cfg-R` (lecture de config vault), `log_reads` porte `P-W†` :
« cannot yet yield Allow ». La note `†` (`spec/04-mandates.md:1818-1819`) ne
rattrape que les mutations, pas les lectures :

> The config-read evidence encoding is WIP; config mutations are always
> journalized independently of `log_reads`.

### (2) Ce que le code fait de chaque côté

Les deux comportements coexistent.

- Côté §07.9.2 pour les lectures d'Ethos : `log_reads` est **type-validé** puis
  ignoré. `rust/crates/aithos-core/src/constraints.rs:941` :
  ```rust
  "log_reads" | "disclose_agency" | "first_party_only" => want_true(key, v)?,
  ```
  et `rust/crates/aithos-core/src/constraints.rs:1385` ne fait que constater la
  présence pour l'atténuation. Aucun chemin de lecture d'Ethos n'émet
  d'`ethos.read` en fonction du drapeau. Un `ethos.read` d'Ethos est bien
  journalisé avec corps scellé — `rust/crates/aithos-bundle/src/log.rs:493` :
  « A journalized read (§07.9.2, `log_reads`): sealed body naming the … » — mais
  sans que le drapeau conditionne quoi que ce soit.
- Côté opposé, pour la lecture de config vault :
  `rust/crates/aithos-bundle/src/vault.rs:249-263` émet **inconditionnellement**
  une entrée `Kind::EthosRead`, sans consulter `log_reads`, et
  `rust/crates/aithos-bundle/src/vault.rs:196-202` lui donne un payload **clair**
  avec `body_enc: None` (`:202`) :
  ```rust
  payload: Some(serde_json::json!({
      "after": after,
      "before": before,
      "operation": operation,
      "record_key": Self::config_record_key(connector),
  })),
  body_enc: None,
  ```
  Le commentaire `rust/crates/aithos-bundle/src/vault.rs:213` l'assume :
  « Reads are journalized. »

Deux règles de §07.9.2 sont donc violées simultanément par ce chemin — l'entrée
existe hors mandat `log_reads`, et son corps n'est pas scellé — pendant que la
cellule `P-W†` de §04.13 exigeait un fail-closed.

### (3) Classe

`LES DEUX IMPLÉMENTÉS`. Le chemin Ethos suit §07.9.2 (corps scellé) mais sans la
condition `log_reads` ; le chemin vault-config viole la condition **et** la forme
du payload, là où §04.13 demandait de refuser. Arbitrer veut supprimer un des deux
comportements.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Deux options exclusives, à trancher au gate humain :

- **(a)** Aligner le code sur §04.13 : `vault_config_operation` avec
  `VaultConfigOperation::Read` renvoie fail-closed tant que l'encodage
  d'évidence config-read est WIP. Coût faible en lignes, mais casse le
  chemin de lecture de config du gateway et tout scénario Gherkin qui en dépend.
- **(b)** Aligner la spec sur le code : lever la restriction « `ethos.read`
  entries exist only under a `log_reads` mandate » pour le domaine
  `vault-config`, et déclarer explicitement en §07.9.2 que le payload y est clair
  parce que `record_key` est déjà un commitment (`vault.rs:69-73`) et ne divulgue
  ni nom d'enregistrement ni plaintext — ce qui satisfait §08.2:226-229. Coût
  faible ; mais transforme la cellule `P-W†` en `A`, ce qui doit être arbitré
  contre §04.13 et non décidé ici.
- Ce que cela casserait, dans les deux cas : la ligne 1805 de
  `spec/04-mandates.md` et les lignes 340-345 de `spec/07-gamma.md` ne peuvent
  plus coexister telles quelles ; l'option (a) casse en outre un chemin
  fonctionnel réel.
- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle.**

---

## SC-05 — `max_sessions` : tier V en §04.4 contre fail-closed en §04.7/§04.13

**Famille 1 — Contradiction. Moitié spec : classe `TEXTUELLE` sur §04.4.
Moitié code : classe `AUCUN`. Sévérité : moyenne** (justifiée en (3)).

> **Levée d'embargo, sur la moitié code seule.** La moitié spec-contre-spec —
> l'étape (1) ci-dessous — a toujours été publiée en entier ; ses deux côtés sont
> dans `spec/04-mandates.md`, à 1 550 lignes d'écart. La **moitié code** — les
> étapes (2), (3) et (4) — a été retenue du **2026-08-04T07:40Z** au
> **2026-08-04T13:00Z** sous la « condition 9 » de la barrière de divulgation
> (`features/.agents/orchestrator/BLOCKED.md`), au motif que l'écart y est
> permissif et non restrictif. **Levée par le propriétaire (Mathieu Colla) le
> 2026-08-04**, décision consignée dans `features/.agents/orchestrator/BLOCKED.md`
> § « Résolues ».
>
> Comme pour SC-12, le texte remis hors dépôt (`/root/work/EMBARGO-SC-05.md`)
> n'existait plus au moment de la levée. La moitié code ci-dessous est
> **re-dérivée depuis `spec/` et depuis le code à `223924e`**, pas restituée. Un
> élément du résumé survivant ne tient pas tel quel ; il est corrigé en (2).
>
> **Chaque phrase ci-dessous dit de quelle couche elle parle.** C'est la raison
> même du découpage : la moitié spec s'établit dans `spec/` seule et n'a besoin
> d'aucun code ; la moitié code s'établit par lecture de `rust/**` et de
> `vectors/**`, et n'établit rien de ce que `spec/` prescrit.

### (1) Les deux côtés, verbatim — **couche spécification**

`spec/04-mandates.md:238` — table du vocabulaire de contraintes §04.4, colonne
« Tier » :

> | `max_sessions: N` | at most N session keys simultaneously certified by the grantee's long-term key (§4.7) — blocks silent duplication of one mandate across N machines | V |

avec `spec/04-mandates.md:229-231` pour la définition du tier, **lue jusqu'à la
parenthèse fermante** — la version publiée de ce constat citait `:232-233`, qui
est le blanc précédant la table, et s'arrêtait avant la parenthèse ; les deux
sont corrigés ici :

> Each known key states its **enforcement tier**: **V** verifier (offline, from files)
> or **X** executor/tool-host (runtime). (Counter-signature, once its own tier **C**,
> is now the owner instance of an obligation — tier V, §4.12.)

`spec/04-mandates.md:1345-1348` :

> SC1 also does not define session issuance, replacement, revocation, expiry
> indexing, or the public set of simultaneously active sessions. Consequently the
> `max_sessions` lifecycle and counter remain reserved and fail closed until their
> own versioned wire is approved.

`spec/04-mandates.md:1789` (matrice §04.13) :

> | `max_sessions` lifecycle/counter | F | F | F | F | F | F | F | F |

avec `spec/04-mandates.md:1777` :

> **F** = fail closed unconditionally under the current wire;

Un même nom de contrainte est donc à la fois « tier V, appliquée hors ligne
depuis les fichiers » et « fail closed unconditionally » sur les huit colonnes
d'opération.

La contradiction ci-dessus est **entièrement interne à `spec/`** : ses deux côtés
sont déjà publics, dans le même fichier, à 1 550 lignes d'écart. Elle est
publiable telle quelle et n'apprend rien à personne qui n'ait déjà lu §04.

### (2) Ce que le code fait de chaque côté — **couche code, moitié re-dérivée**

**Aucun des deux côtés de l'étape (1) n'est implémenté.** Le tier `V` de §04.4
n'a aucun chemin d'application en production, et le `F` de §04.13 n'a aucun
chemin de refus. Le résultat net est que le code **accepte** — il ne compte pas,
et il ne refuse pas.

- **Le mot-clé est connu et typé.** `rust/crates/aithos-core/src/constraints.rs:923`,
  dans `validate_known_constraints` :
  ```rust
  "max_actions" | "max_children" | "max_sessions" | "max_mutations"
  | "max_consumptions" => {
      want_u64(key, v, None)?;
  }
  ```
  Un lien de délégation portant `max_sessions: N` bien formé est donc **valide de
  forme** — il n'est ni inconnu (donc pas soumis au fail-closed M0.c de §04.4
  pour clé inconnue), ni réservé, ni refusé.
- **Il est même atténué comme les autres compteurs.**
  `rust/crates/aithos-core/src/constraints.rs:1341-1346`, dans
  `family_containment` : le plafond de l'enfant ne peut pas dépasser celui du
  parent. La chaîne complète de `verify_chain` (`mandate.rs:1111-1118`) l'exécute
  à chaque lien. Le code traite donc `max_sessions` exactement comme
  `max_actions` : une contrainte vivante, pas une contrainte réservée.
- **Le compteur existe, et rien en production ne l'appelle.**
  `rust/crates/aithos-core/src/constraints.rs:1293-1316`, `verify_max_sessions`,
  dont le commentaire de doc (`:1291-1292`) est explicite sur ce qu'il ne fait
  pas : « Enforce `max_sessions` over an injected set of already verified active
  session public keys. This adds no certificate wire or storage rule. »
  L'ensemble des sessions actives est **injecté** par l'appelant ; la fonction
  n'a aucun moyen de le découvrir depuis les fichiers, ce que §04.4:229-231 exige
  pourtant d'un tier `V` (« offline, from files »).

**L'affirmation d'absence, avec sa recherche, son périmètre et sa couche.**
`grep -rn "verify_max_sessions" .` — périmètre : **dépôt entier** à `223924e`,
donc les cinq crates de l'espace de travail (`rust/Cargo.toml:3-9` :
`aithos-core`, `aithos-bundle`, `aithos-cli`, `aithos-owner`, `aithos-wasm`),
`vectors/`, `docs/`, `features/`, `scripts/` — couche **code et corpus de
données**. Résultats, en entier :

- la définition, `rust/crates/aithos-core/src/constraints.rs:1293` ;
- un `use` et **trois** sites d'appel, tous dans un seul fichier de **test** :
  `rust/crates/aithos-core/tests/cb5_evidence_contracts.rs:3`, `:121`, `:124`,
  `:128` ;
- de la prose : `docs/research/topology-2026-07-28-unverified/lot-C-04.md:161`,
  `:341`, `:532`, `:600` ; `docs/archive/HANDOFF-GATEWAY-G4-PROD-MCP-DELEGATED-SESSIONS-2026-07-22.md:221`,
  `:372`, `:590`, `:782` ; `vectors/cb2-core-bundle-red-ledger.json:2205` ;
- et `docs/audits/split/spl8-amputation.patch:22340` et `:28833`, qui **retirent**
  (`-use aithos_core::constraints::verify_max_sessions;`,
  `-    verify_max_sessions(max_sessions, active_session_keys)`) le dernier
  appelant de production, celui du crate `gateway` amputé du dépôt.

**Aucun fichier sous `rust/crates/*/src/**` n'appelle `verify_max_sessions`.**
Même résultat pour son type de retour : `grep -rn "VerifiedSessionTally" .` ne
renvoie que sa propre définition et la signature de la fonction
(`constraints.rs:1280`, `:1284`, `:1296`, `:1314`). Cette recherche est
**syntaxique** : elle n'exclut pas, à elle seule, un appel dynamique ou une
réexportation qu'une lecture aurait manqués.

**Corroboration mesurée.** Le test qui porte les trois appels est **vert** à
`223924e` — `ev-fafd51d8`, `cargo test -p aithos-core --test
cb5_evidence_contracts`, 5 tests passés — et l'espace de travail entier l'est aussi —
`ev-cb4ff302`, 18 features / 114 règles / 836 scénarios / 3 577 étapes, tests
unitaires compris. Ces deux transcriptions établissent que `verify_max_sessions`
est bien exercée, qu'elle passe, et qu'elle l'est **depuis la couche test**. Le
vert **est** le constat, il ne le réfute pas : la fonction fait exactement ce que
son test lui demande, et rien ne la demande ailleurs. Ce que les transcriptions ne
peuvent pas faire, c'est prouver une absence — le drapeau de lecture reste levé
sur l'affirmation « aucun `src/` ne l'appelle », qui est établie par recherche
syntaxique et par elle seule.

> **Correction au dossier survivant.** `BLOCKED.md` et le résumé de la passe
> parlent d'« une fonction sans appelant ». C'est imprécis et il faut le dire :
> `verify_max_sessions` **a** trois appelants. Ils sont tous les trois dans
> `rust/crates/aithos-core/tests/cb5_evidence_contracts.rs`, c'est-à-dire dans la
> couche test. L'énoncé exact est : *aucun `src/` d'aucun crate de l'espace de
> travail ne l'appelle*. La différence compte, parce qu'un test vert donne
> l'apparence d'une contrainte appliquée — et
> `cb5_evidence_contracts.rs:112-131` est nommé
> `cb5_max_sessions_lifecycle_reaches_the_typed_validator`, ce qui est vrai du
> validateur et faux de tout chemin de consommation.

**La direction de l'écart — c'est le fait décisif, et il est permissif.** Trois
observations indépendantes, couche code puis couche corpus de données :

1. **Un émetteur de production émet des mandats qui en portent.**
   `rust/crates/aithos-owner/src/lib.rs:804-816` : le délégué de session est
   frappé avec `serde_json::json!({ "max_sessions": 3, "purpose": … })`, stocké
   sous `certs/`, puis journalisé par `log_owner_grant` (`:825-827`). Aucun
   refus, aucun avertissement, aucune trace de « réservé ».
2. **Le vecteur normatif gèle l'acceptation.**
   `vectors/cb2-mandate-contracts.json`, cas `"all known families well-formed"`
   (`:5`), contient `"max_sessions": 1` (`:68`) et porte
   `"expected_shape_valid": true` (`:96`). Le seul cas négatif du vecteur pour
   cette clé est `"malformed max_sessions"` avec `-1` (`:113-117`,
   `expected_shape_valid: false`). Autrement dit : la valeur **mal formée** est
   refusée, la valeur **bien formée** est acceptée. C'est l'inverse exact d'un
   `F`.
3. **Les vecteurs de chaîne de session verrouillent la même chose.**
   `vectors/cb14-delegated-session-chain.json` porte `max_sessions: 3` dans la
   racine et dans la feuille de chaque chaîne positive (`:41`, `:90`, `:190`,
   `:219`, `:268` …), et son `inventory.negative_ids` ne liste que
   `truncated-chain`, `revoked-parent`, `substituted-leaf`,
   `crossed-session-proof`, `verification-time-mismatch` — **aucun** cas négatif
   `max_sessions`. Idem pour `vectors/cb15-external-delegated-grant.json:22`,
   `:72`, `:144`.

Ce que §04.13:1789 demandait — « fail closed unconditionally » sur les huit
colonnes d'opération, pour une consommation de **grantee** (`spec/04-mandates.md:1780-1781`) —
n'existe nulle part. Un mandat qui déclare « au plus 3 sessions simultanées » est
consommé sans qu'aucune session ne soit comptée, sur toutes les opérations. La
promesse de §04.4:238 est donc opposable à un lecteur de `spec/` et sans effet
dans le code, et la réserve de §04.7:1345-1348 est opposable à un lecteur de
`spec/` et sans effet non plus.

**Honnêteté sur la lecture de `F`.** On peut lire la ligne `max_sessions`
*lifecycle/counter* de §04.13 étroitement — « c'est le compteur qui échoue fermé,
et comme rien n'en dépend, rien n'échoue ». Cette lecture sauve le code de la
contradiction avec §04.13 mais le laisse en pleine contradiction avec §04.4:238,
qui range `max_sessions` en tier `V`, c'est-à-dire appliqué « offline, from
files » par le vérificateur. Sous l'une ou l'autre lecture, le comportement
observé est le même et il est permissif. C'est pourquoi il n'a pas été publié en
même temps que l'étape (1) : `spec/` avertit son lecteur que `max_sessions` est
réservé ; elle ne lui dit pas qu'un mandat qui en porte un est **accepté et
consommé**.

### (3) Classe et sévérité — **couche code**

**`AUCUN`.** Le classement est celui de SC-03, SC-07 et SC-09, pas celui de
SC-01 : il n'y a pas deux comportements dans le code entre lesquels arbitrer, il
y a **deux énoncés normatifs et zéro implémentation**. Le tier `V` de §04.4:238
n'a aucun chemin d'application ; le `F` de §04.13:1789 n'a aucun chemin de refus.
C'est cette classe, et non le détail des `fichier:ligne`, qui divulguait la
direction de l'écart : dire « aucun des deux côtés n'est implémenté » sur une
paire dont un côté est un refus, c'est dire que rien ne refuse.

**Sévérité : moyenne.** Elle ne monte pas plus haut parce que la contrainte est
*facultative* — elle ne relâche rien de ce que le mandat interdit par ailleurs,
et un mandat sans `max_sessions` n'est pas plus permissif qu'un mandat qui en
porte un. Le dommage est un dommage de **fausse assurance** : un émetteur, y
compris `aithos-owner` lui-même (`lib.rs:812`), croit borner la duplication d'un
mandat sur N machines et ne la borne pas ; §04.4:238 lui a promis un tier `V`.
Elle ne descend pas plus bas parce que la contrainte est gelée dans des vecteurs
normatifs comme acceptée, ce qui la rend coûteuse à durcir plus tard qu'à durcir
maintenant.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

La moitié spec et la moitié code se corrigent séparément et dans cet ordre.

**Moitié spec (indépendante, gratuite).** La contradiction §04.4:238 ↔
§04.13:1789 est un défaut de spécification pur. Retirer le tier `V` de
`spec/04-mandates.md:238` et le remplacer par une mention explicite de réserve
renvoyant à §04.7:1345-1348 — ou, symétriquement, retirer la ligne réservée de
§04.13:1789. Coût faible, une ligne, aucun octet signé, aucun code.

**Moitié code.** Deux options exclusives, à trancher au gate humain :

- **(a) Appliquer §04.13 à la lettre : refuser.** Sortir `max_sessions` de la
  liste de `constraints.rs:923` et lui donner sa propre branche qui échoue fermé
  à la validation de lien, comme le fait déjà `validate_profile_constraints`
  (`constraints.rs:1319-1336`) pour `max_mutations`/`max_consumptions` hors
  `draft.3`. Coût faible en lignes ; mais cela casse
  `rust/crates/aithos-owner/src/lib.rs:812` (à retirer), le vecteur
  `vectors/cb2-mandate-contracts.json` (cas positif à régénérer),
  `vectors/cb14-delegated-session-chain.json` et
  `vectors/cb15-external-delegated-grant.json` (chaînes entières à régénérer),
  et le test `cb5_evidence_contracts.rs:112-131`. C'est un vrai coût au sens de
  `features/AGENTS.md` § *Project stage* : il porte sur les vecteurs et les tests
  **du dépôt**, pas sur des détenteurs qui n'existent pas.
- **(b) Appliquer §04.4 à la lettre : compter.** Définir le wire de cycle de vie
  de session que §04.7:1345-1348 déclare manquant — émission, remplacement,
  révocation, indexation d'expiration, ensemble public des sessions actives — puis
  brancher `verify_max_sessions` sur cet ensemble reconstruit depuis les fichiers,
  et non injecté. Coût lourd : nouveau construit signé, donc nouveau profil au
  sens de §00.4:74-84, nouveaux vecteurs. C'est exactement le travail que
  §04.7:1345-1348 met en réserve, et le trancher ici serait décider un point de
  protocole.
- **Variante minimale, à arbitrer contre les deux :** ne rien changer au wire et
  écrire dans §04.4:238 ce que le code fait — `max_sessions` est parsé, typé et
  atténué, jamais appliqué — comme §04.13 le fait déjà pour d'autres lignes avec
  le suffixe `W`. Coût texte faible ; mais cela grave une contrainte inerte dans
  le vocabulaire normatif, ce que §04.4 reproche ailleurs aux extensions
  inconnues (bloc `CB1 decision G-E`, `spec/04-mandates.md:214-227`).

**Critère de clôture** — attribuable depuis la ligne de base `ev-cb4ff302`
(espace de travail vert à `223924e`) et `ev-fafd51d8` (`cb5_evidence_contracts`
vert, 5 tests). Sous (a) : un test RED démontre qu'un mandat de
délégation portant `max_sessions` bien formé est refusé par `verify_chain`, là où
il est aujourd'hui accepté, et les vecteurs cités sont régénérés. Sous (b) :
`verify_max_sessions` est appelée depuis au moins un `src/` de crate, avec un
ensemble de sessions actives reconstruit depuis le store, et un test RED démontre
qu'une (N+1)-ième session est refusée sans que l'appelant ait à injecter la liste.
Dans les deux cas, la recherche `grep -rn "verify_max_sessions" rust/crates/*/src/`
cesse d'être vide, ou la fonction disparaît.

- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle**, et aucune
  commande `cargo`, `git` ou de test n'a été lancée pour ce constat.

---

## SC-06 — Le repli « exactly-N » de §03.4 est rejeté par la règle de rotation du code

**Famille 3 — Inatteignable. Classe : `INATTEIGNABLE`. Invariant touché : I3.**

C'est le constat le plus lourd de la passe : la phrase est claire, elle n'a pas de
contradictoire dans `spec/`, et aucun chemin du code ne permet de s'y conformer.

### (1) L'énoncé, verbatim

`spec/03-headers.md:87-95` (étape 2bis de la rotation) — **lu jusqu'au point**,
car c'est la dernière subordonnée qui porte le cas :

> 2bis. Derivation up-link. If the rotated node N is derived from a parent node P that
>    the rotator holds, it also publishes an up-link wrap: seal(DK'_N) openable via
>    K_P — same primitive as a tag wrap (AAD purpose `tagwrap`, §00.3), bound to
>    subject_did ‖ N ‖ new key_version. The wrap restores the parent→child derivation
>    path broken by the fresh random DK', so holders of P (or of any ancestor of P)
>    keep reading N by derivation without needing a line of their own. If the rotator
>    holds exactly N but not P, it instead seals DK'_N individually to the current
>    holders of P (public keys read from P's header); the first manager of P that
>    later acts posts the definitive wrap.

et `spec/03-headers.md:108-113`, qui en fait une règle de vérification :

> Verification is mechanical: the new version's lines MUST equal the previous lines
> minus the revoked (plus, in the exactly-N case, recipients ⊆ P's header), the new
> version MUST carry the owner line as defined in §3.1 — the revoker re-seals DK' to
> the subject's `owner_kex` read from the DID document, never to whatever key the
> previous owner line used — and an up-link wrap whose author does not hold P is
> rejected.

La parenthèse est explicite : dans le cas *exactly-N*, la nouvelle version porte
légitimement des destinataires **absents de la version précédente de N** — ceux
lus dans le header de **P**.

### (2) Ce que le code fait

`rust/crates/aithos-core/src/header.rs:334-363`, `Header::check_rotation`,
implémente la règle générale et **rejette précisément le cas d'exception** :

```rust
let prev_kids: std::collections::BTreeSet<&str> =
    prev.lines.iter().map(|l| l.kid.as_str()).collect();
for line in &new.lines {
    if !prev_kids.contains(line.kid.as_str()) {
        return Err(err(format!(
            "{}: rotation smuggles in recipient {}",
            self.node, line.kid
        )));
    }
}
```

Tout `kid` de la nouvelle version absent de la version précédente est un
« smuggled recipient », sans exception. Le commentaire de doc `header.rs:328-333`
énonce la règle générale seule et ne mentionne pas le cas *exactly-N*. Aucune
autre fonction ne valide une rotation : `grep -rn "check_rotation" rust/ --include=*.rs`
ne renvoie que la définition `header.rs:334`, deux appels producteurs
(`rust/crates/aithos-bundle/src/revoke.rs:214`,
`rust/crates/aithos-bundle/src/vault.rs:404`) et trois usages de test
(`rust/crates/aithos-bundle/tests/cucumber.rs:15292`,
`rust/crates/aithos-core/tests/g2_rotation.rs:97,114`).

Le chemin producteur lui-même n'implémente pas non plus le repli : la rotation de
dossier construit son wrap depuis la clé de zone,
`rust/crates/aithos-bundle/src/revoke.rs:217-219` :
```rust
let zone_dk = self.zone_dk(zone, owner)?;
```
c'est-à-dire le cas « le rotateur détient P », jamais le cas « il détient
exactement N ».

Le vecteur normatif ne couvre pas davantage l'exception : `vectors/g2-rotation.json`
n'a que les champs `old_kids`, `revoked_kid`, `expected_survivor_kids`,
`smuggled_new_kid`, `smuggled_must_fail`, `missing_owner_must_fail`, `uplink` —
aucun cas *exactly-N* (couche corpus de données, fichier lu intégralement).

### (3) Classe

`INATTEIGNABLE`. Un rotateur qui détient exactement N et pas P **doit**, selon
§03.4, sceller DK'_N aux détenteurs de P lus dans le header de P. Ces
destinataires ne sont pas dans la version précédente de N — c'est toute la raison
d'être du repli. `check_rotation` les rejette donc systématiquement. La phrase de
§03.4 est claire, rien dans `spec/` ne la contredit, et aucun chemin du code ne
permet de s'y conformer : elle ne se voit ni en lisant la spec seule, ni en lisant
le code seul.

Portée : I3 est préservé dans les deux lectures (le repli conserve la ligne
owner), donc l'inatteignabilité ne crée pas de trou I3 — elle rend inaccessible
le mécanisme qui, selon §03.4:105-107, existe précisément pour qu'« a holder of
the zone who read N by pure derivation (§02.5) would silently lose N without
having any header line to fall back on » ne se produise pas dans le cas absentee-owner.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Donner à `check_rotation` le paramètre manquant : la liste des `kid` du header de
P, et accepter un `kid` nouveau si et seulement s'il appartient à cet ensemble —
la parenthèse de §03.4:109 traduite littéralement.

- Coût estimé : moyen. Signature de `check_rotation` changée (deux appelants
  producteurs à mettre à jour), plus l'implémentation du chemin producteur
  *exactly-N* qui n'existe pas du tout aujourd'hui.
- Ce que cela casserait : la signature publique de
  `aithos_core::header::Header::check_rotation` est un point d'API cassé pour
  tout appelant externe. `vectors/g2-rotation.json` devrait recevoir un cas
  *exactly-N* positif, ce qui est une extension de vecteur normatif au sens de
  §09.2 (« normative at promotion ») et relève donc du gate.
- Variante à arbitrer contre la précédente : supprimer le repli de §03.4 et
  déclarer que la rotation d'un node dérivé exige la détention de P. Coût texte
  faible, mais réintroduit dans le profil absentee-owner (§00.5) exactement la
  dépendance récursive que §03.4:105-107 dit vouloir éviter.
- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle.**

---

## SC-07 — La révocation par omission n'est vérifiée nulle part

**Famille 2 — Lettre morte. Classe : `AUCUN`. Invariant touché : I4.**

### (1) L'énoncé, verbatim

`spec/05-delegation.md:101-103` :

> A verifier rejects a header rotation whose signer is not an authorized issuer for the
> lines it changed, or that drops a line the signer had no authority over (that would be
> an unauthorized revocation by omission).

`spec/10-threat-model.md:117-119` en fait une exigence de revue pré-promotion :

> External review MUST cover: derivation label domain separation; header-line AAD
> binding and the owner-line invariant; the authority-to-rotate check (§05.5) against
> unauthorized revocation-by-omission; […]

### (2) Ce que le code fait

Le sens `⊆` (pas d'intrus) est implémenté ; le sens `⊇` (pas de ligne supprimée
sans autorité) ne l'est pas, et rien n'est appelé côté vérificateur.

- `rust/crates/aithos-core/src/header.rs:347-356` ne parcourt que `new.lines` et
  ne teste que l'appartenance de chaque nouveau `kid` à `prev_kids`. Aucune
  boucle inverse sur `prev.lines` ne cherche un `kid` disparu. Une rotation qui
  supprime dix lignes légitimes et n'en ajoute aucune passe `check_rotation` sans
  erreur.
- Le signataire de la rotation n'entre pas dans la fonction : `check_rotation`
  ne reçoit que `new_version: u64` et `owner_kid: &str`. Elle n'a donc aucun
  moyen de statuer sur « an authorized issuer for the lines it changed ».
- `check_rotation` n'est appelée que par les deux chemins **producteurs**
  (`revoke.rs:214`, `vault.rs:404`), immédiatement après que ces mêmes chemins ont
  eux-mêmes construit le tableau `survivors` (`revoke.rs:180-200`,
  `vault.rs:373-392`). C'est une assertion sur son propre calcul, pas une
  vérification d'un header présenté par un tiers. Le vérificateur d'édition
  (`rust/crates/aithos-bundle/src/bundle.rs:302-326`, `verify_pinned_headers`)
  n'appelle que `header.validate(owner_kid)` — l'inventaire I3 — et jamais
  `check_rotation` (`bundle.rs:318`).

### (3) Classe

`AUCUN`. La spécification décrit un comportement de vérificateur — refuser une
rotation qui laisse tomber une ligne hors de l'autorité du signataire — que ni la
couche `aithos-core` ni la couche `aithos-bundle` n'implémentent, dans aucune
direction. Ce n'est pas un côté d'une contradiction : c'est un énoncé normatif
sans implémentation, et §10.9 le désigne comme un point de revue externe
obligatoire.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Étendre `check_rotation` en `check_rotation_authority(prev, new, signer_chain,
owner_kid)` : pour chaque `kid` présent dans `prev` et absent de `new`, exiger que
la chaîne du signataire couvre le mandat qui détenait cette ligne, en réutilisant
le contrôle d'autorité de révocation déjà spécifié en §06.4 plutôt qu'en écrivant
une seconde règle d'autorité. Puis l'appeler depuis `verify_pinned_headers`.

> **Pointeur restauré le 2026-08-04.** Ce paragraphe portait la mention « le site
> d'implémentation de ce contrôle n'est pas nommé ici : il coïncide avec l'un des
> deux sites de SC-12, retenu pour divulgation ». La relecture confirme que ce
> retrait n'avait **aucun autre motif** que la protection de SC-12, dont l'embargo
> est levé. Le contrôle d'autorité de révocation de §06.4 est
> `aithos_core::revocation::check_revoke_authority`,
> `rust/crates/aithos-core/src/revocation.rs:57-103`. Rien d'autre de ce constat
> n'est modifié — et il faut lire ce renvoi avec SC-12 : le contrôle qu'il propose
> de réutiliser est lui-même défectueux sur sa branche de portée nue.

- Coût estimé : lourd. Le vérificateur d'édition ne dispose aujourd'hui d'aucune
  liaison `kid → mandat` : `verify_pinned_headers` ne reçoit que `store`, `files`
  et `doc` (`bundle.rs:302-306`). Il faudrait joindre les certificats de `certs/`
  au header, ce qui est une nouvelle dépendance de vérification, et le faire sans
  clé (contrainte de §09.4:101-102).
- Ce que cela casserait : toute édition historique dont une rotation a supprimé
  une ligne sans que la chaîne du signataire soit reconstituable depuis `certs/`
  deviendrait invalide rétroactivement. §00.4:88-92 n'autorise **qu'un seul**
  durcissement rétroactif dans cette série, celui d'I3 ; en introduire un second
  est une décision de gate, pas d'implémentation.
- **La faisabilité de cet arbitrage n'a pas été vérifiée par ce rôle.**

---

## SC-08 — L'autorité de l'auteur d'un up-link wrap n'est pas déterminable depuis le wire

**Famille 3 — Inatteignable. Classe : `INATTEIGNABLE`. Invariant touché : I4.**

### (1) L'énoncé, verbatim

`spec/03-headers.md:108-113`, dernière clause :

> […] and an up-link wrap whose author does not hold P is rejected.

`spec/10-threat-model.md:118-119` :

> the up-link wrap authority check (§03.4) against unauthorized re-linking;

`spec/09-cli-and-conformance.md:54-56` en exige un vecteur :

> Session-2 additions MUST
> also be covered: an up-link wrap open after rotation (and rejection of a wrap by a
> non-holder of the parent); […]

### (2) Ce que le code fait

L'objet `Wrap` ne porte **ni auteur, ni clé, ni signature** :
`rust/crates/aithos-core/src/header.rs:406-415` :

```rust
pub struct Wrap {
    pub object: String,
    pub node: String,
    pub key_version: u64,
    pub via: String,
    pub n: String,
    pub c: String,
}
```

`Wrap::seal` (`header.rs:417-437`) prend `via_key: &[u8; 32]` et n'enregistre rien
de l'identité de l'appelant. `Wrap::open` (`header.rs:439-447`) ne fait qu'un
`wrap_open` AEAD. Aucun champ du wire ne permet à un vérificateur de nommer
l'auteur, donc aucun vérificateur ne peut décider s'il « holds P ».

Recherche `grep -rni "holds P\|holder of the parent\|wrap author\|wrap_author"
rust/crates/ --include=*.rs` : **aucun résultat**, périmètre `rust/**` complet,
couche code.

Le seul test de la propriété est physique et prouve autre chose :
`rust/crates/aithos-bundle/tests/cucumber.rs:15298-15320`
(« someone without the parent key posts an up-link wrap ») écrase le wrap avec un
scellement sous `&[0xEEu8; 32]`, « not the real zone key » — et l'assertion porte
sur l'échec de l'ouverture, pas sur un rejet de vérificateur. Un wrap illégitime
ne s'ouvre pas ; ce n'est pas la même chose qu'« est rejeté ».

Le vecteur exigé par §09.2 n'existe pas : `vectors/g2-rotation.json` a un champ
`uplink` qui ne contient que `via_key_hex`, `new_dk_hex`, `nonce_hex`, `node`,
`key_version`, `subject_did`, `cipher_hex` — un cas positif d'octets, aucun cas
négatif d'auteur (couche corpus de données, fichier lu intégralement). Aucun
fichier de `vectors/` ne contient « non-holder » (recherche
`grep -rli "non-holder\|non_holder" vectors/*.json` : aucun résultat).

### (3) Classe

`INATTEIGNABLE`. La règle exige de rejeter un wrap sur un critère — l'identité de
son auteur — que le format de wire ne transporte pas. Aucun chemin du code ne
permet de s'y conformer, et aucune implémentation ne le pourrait sans changer le
wire. La propriété effectivement obtenue (un wrap mal scellé ne s'ouvre pas) est
une propriété différente : elle ne dit rien d'un détenteur de K_P qui poste un
wrap pour un node qu'il n'a pas autorité à re-lier.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Deux options exclusives :

- **(a)** Reformuler §03.4 pour énoncer la propriété réelle : « a wrap not sealed
  under K_P does not open and conveys nothing ; wrap authorship is enforced by
  physics, not by a verifier check », et retirer la clause correspondante de
  §10.9 ainsi que l'exigence de vecteur de §09.2:55-56. Coût texte faible ;
  affaiblit une propriété annoncée dans le modèle de menace.
- **(b)** Ajouter au `Wrap` un membre `author` + `sig`, et vérifier la couverture
  de P par la chaîne de l'auteur. Coût lourd : changement de wire additif,
  nouveau profil, régénération de `vectors/g2-rotation.json` et
  `vectors/g3-move.json`, et impact sur `§02.10` — le hash de vue de tag
  `mroot(wraps, by section sid)` (`spec/02-content-tree.md:575`) hache
  `BLAKE3(JCS(wrap))` (`:594`), donc **toute racine d'état déjà signée change**.
- Ce que cela casserait : (a) réduit une garantie publiée dans §10.9 ; (b) casse
  toutes les racines d'état `circle`/`public` déjà signées et impose une
  migration d'édition.
- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle.**

---

## SC-09 — La passe de réparation de wraps de tag n'existe pas

**Famille 2 — Lettre morte. Classe : `AUCUN`.**

### (1) L'énoncé, verbatim

`spec/02-content-tree.md:49-55` — **lu jusqu'au point**, la subordonnée finale
portant toute l'obligation :

> Mutating a section's **tag set** is an edit of the section, never of the tag view: a
> `tag=` grant covers the sections currently carrying the tag, NOT re-labelling. Adding
> or removing a tag requires an `id=`, `dir=` or zone-level edit perimeter on the
> section itself. And when a repair pass creates a missing tag wrap for a section newly
> carrying a tag, it MUST first validate the author of that tag mutation (a covering
> edit perimeter at the mutation entry's `at`, per gamma) and fail closed: an
> unauthorized re-label is never bridged into a tag view.

`spec/09-cli-and-conformance.md:60-61` en exige le vecteur :

> an unauthorized tag
> re-label not bridged by the repair pass; […]

### (2) Ce que le code fait

Aucune passe de réparation n'existe. `grep -rni "repair" rust/crates/*/src/*.rs` :
**aucun résultat** (périmètre : tous les fichiers source des crates, couche code).

La création de wraps de tag est faite en ligne, au moment de la mutation, par le
chemin structurel : `rust/crates/aithos-bundle/src/structure.rs:861-882` scelle un
`Wrap` pour chaque anchor de tag dont la section porte l'étiquette, au sein de la
transaction qui autorise déjà l'auteur. Il n'y a donc jamais de wrap « manquant »
à réparer *dans ce chemin*, mais il n'y a pas non plus de chemin de rattrapage
pour les wraps manquants d'une autre origine (import, merge, réencryption
paresseuse), qui est exactement le cas que §02.2 régit.

Aucun vecteur ne couvre le cas : `grep -rli "relabel\|re-label" vectors/*.json` :
aucun résultat (couche corpus de données).

### (3) Classe

`AUCUN`. L'énoncé normatif décrit un mécanisme — la passe de réparation et son
contrôle d'autorité fail-closed — qui n'a d'implémentation d'aucun côté, et dont
le vecteur exigé par §09.2 est absent.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Deux options exclusives :

- **(a)** Retirer la clause de §02.2:52-55 et l'exigence de vecteur de
  §09.2:60-61, en actant que la création de wraps est toujours en ligne
  (`structure.rs:861-882`) et qu'une passe de réparation hors transaction n'est pas
  dans cette édition. Coût texte faible.
- **(b)** Implémenter la passe et son contrôle. Coût lourd : elle doit relire le
  gamma pour retrouver l'entrée de mutation de tag, en extraire l'`at` et
  l'`authorized_via`, et évaluer un périmètre d'édition couvrant à cette date —
  or sur `circle` et `self` la cible de l'entrée est **scellée** (§07.3:120-127),
  donc la passe ne peut le faire qu'en détenant la clé du node.
- Ce que cela casserait : (a) supprime une garantie annoncée ; (b) crée une passe
  de maintenance qui exige des clés de contenu, ce qui la met hors du profil
  « recursive maintenance » de §00.5:128-133 pour un détenteur sans ligne.
- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle.**

---

## SC-10 — §01.4 : le signataire de la transition d'époque, énoncé puis contredit dans la même phrase

**Famille 1 — Contradiction. Classe : `TEXTUELLE`.**

### (1) Les deux côtés, verbatim

`spec/01-identity-and-keys.md:116-119`, un seul paragraphe, cité intégralement
jusqu'au point final — c'est précisément ici que lire jusqu'au bout change la
conclusion :

> A verifier accepts a successor DID document only if the transition verifies under
> the **previous** document's `succession` key. Any other signer — including
> `#root` itself — is rejected: a stolen `S` can never steal the identity's future. It is signed by root_sign and versioned by the same
> edition chain as the bundle. Grantee keys never appear in it.

La troisième phrase (« It is signed by root_sign ») suit immédiatement le rejet
explicite de `#root`. L'antécédent de « It » est ambigu : le document DID
successeur (signé `#root` par le bloc JSON de `:98`) ou la transition d'époque
(signée `#succession` par le bloc de `:113`). Le paragraphe entier porte sur la
transition ; l'anomalie de mise en forme de la ligne 118 (retour à la ligne
manquant, ligne anormalement longue face au reste du fichier) indique une
insertion qui a déplacé une phrase appartenant au paragraphe du document DID.

`spec/01-identity-and-keys.md:36-39` renforce l'exclusivité :

> It is the **sole authority** for one act: declaring a new
> master key, i.e. signing the identity-epoch transition (§10.4) that publishes a
> successor DID document when `S` is compromised or lost. […] It signs
> nothing else, ever.

### (2) Ce que le code fait de chaque côté

Un seul côté est implémenté : `#succession` exclusivement.

`rust/crates/aithos-core/src/did.rs:20` fige le fragment :
```rust
pub const SUCCESSION_FRAGMENT: &str = "#succession";
```
et `did.rs:236-256` rejette tout autre signataire — `:239` :
« a transition must be signed by the succession key » — puis `:252-256` vérifie
sous `prev_doc.succession_pub()`. `did.rs:222-226` documente qu'une déclaration
signée autrement n'est pas une transition canonique.

Le côté « signed by root_sign » n'a aucune implémentation pour la transition.
Il en a une pour le **document DID** (`did.rs:114`, `keys.root`), ce qui confirme
que la phrase est un reliquat du paragraphe précédent.

### (3) Classe

`TEXTUELLE`. Le côté « root_sign » est lettre morte pour la transition. Correction
bon marché — mais la phrase se trouve dans les quatre lignes qui décrivent le seul
chemin de sortie d'un `S` compromis (§10.8:111-112), c'est-à-dire là où une
mauvaise lecture est la plus coûteuse.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Déplacer « It is signed by root_sign and versioned by the same edition chain as the
bundle. Grantee keys never appear in it. » dans le paragraphe du document DID
(après `spec/01-identity-and-keys.md:103`), où son antécédent est sans ambiguïté.

- Coût estimé : faible. Deux phrases déplacées dans `spec/01-identity-and-keys.md`.
  Aucun octet signé, aucun changement de code.
- Ce que cela casserait : rien.
- **La faisabilité de cet arbitrage n'a pas été vérifiée par ce rôle.**

---

## SC-11 — Emplacement canonique du node vault, et angle mort I3 du prédicat de header

**Famille 3 — Inatteignable. Classe : `INATTEIGNABLE`. Invariant touché : I3.**

### (1) Les deux côtés, verbatim

`spec/02-content-tree.md:70` (layout canonique §2.3) :

```
x/<id>/…                   vault, §08
```

`spec/02-content-tree.md:22` (chemins canoniques §2.1) :

```
/x/<connector>                  vault node (DK, header) — §08
```

`spec/03-headers.md:8-12` :

> One header per granted node — a zone root `/e/<zone>`, any folder
> `/e/<zone>/d/<sid>/…`, any tag view `…/t/<tag>` (zone-root or folder-local), any
> section `…/s/<sid>`, or a vault `/x/<id>` — at `.../header.json`.

`spec/00-overview.md:35-40` (I3) et `spec/09-cli-and-conformance.md:99-102` :

> - **Core reader**: resolves DID, opens headers it has lines for, derives, decrypts,
>   verifies editions + gamma. MUST implement the fork rule (§02.6) fail-closed, and
>   MUST reject an edition pinning a header that violates I3 (§03.1) — without holding
>   any key, and on every `aithos-core` manifest profile.

### (2) Ce que le code fait de chaque côté

Les deux espaces de clés existent, mais le header vault est écrit dans le
**mauvais** au regard de §02.3, et le prédicat I3 est indexé sur cette erreur.

- Le header vault est écrit sous `e/x/…`, pas sous `x/…` :
  `rust/crates/aithos-bundle/src/vault.rs:55-63` :
  ```rust
  fn config_header_path(connector: &str) -> String {
      format!("e/x/{connector}/header.json")
  }
  fn config_blob_path(connector: &str) -> String {
      format!("e/x/{connector}/manifest.enc")
  }
  ```
  alors que le **node logique** reste `/x/<connector>` (`vault.rs:65-67`), ce qui
  est correct pour l'AAD et pour §02.1.
- `x/<id>/…` est néanmoins une clé de store canonique valide, réservée par
  §08.2:232-238 à l'état d'exécution non secret :
  `rust/crates/aithos-bundle/src/lib.rs:204`
  (`segments[0] == "x" && connector_object_accepted(&segments)`), et
  `connector_object_accepted` (`lib.rs:115-135`) accepte tout dernier segment en
  `.json` ou `.enc` — donc `x/<id>/header.json` est une clé de store légale.
- Le vérificateur I3 d'édition indexe sur le préfixe `e/` :
  `rust/crates/aithos-bundle/src/bundle.rs:291-295` :
  ```rust
  pub(crate) fn is_header_file(path: &str) -> bool {
      path.starts_with("e/")
          && path.ends_with(".json")
          && (path.ends_with("/header.json") || path.contains("/hdr/"))
  }
  ```
  et `verify_pinned_headers` (`bundle.rs:302-326`) ne filtre les fichiers épinglés
  qu'à travers ce prédicat (`bundle.rs:311`).

Conséquence directe : un header épinglé à l'emplacement que §02.3, §02.1 et §03.1
désignent comme canonique pour un vault — `x/<id>/header.json` — est une clé de
store valide, est épinglable dans `files`, et n'est **jamais parsé** par
`verify_pinned_headers`, donc jamais soumis à `Header::validate`. I3 exige
pourtant, en §00.2:37-39, que « An edition verifier MUST parse every header the
edition pins ».

Portée honnête : aujourd'hui aucun chemin de production n'écrit de header à cette
adresse (le vault écrit sous `e/x/…`), donc l'angle mort n'est pas atteint par le
code tel qu'il est. Il l'est par toute implémentation qui suivrait `spec/`.

### (3) Classe

`INATTEIGNABLE`. Une implémentation conforme à §02.3/§02.1/§03.1 place son header
de vault sous `x/<id>/header.json` ; elle est alors, au regard du vérificateur de
référence, une édition dont un header pinné échappe à I3 — c'est-à-dire qu'elle ne
peut pas satisfaire simultanément §02.3 et §09.4. Aucun des deux énoncés n'est
contredit dans `spec/` ; c'est leur conjonction avec le prédicat du code qui est
insatisfiable.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Deux options exclusives :

- **(a)** Corriger §02.3 pour graver `e/x/<id>/header.json` et
  `e/x/<id>/manifest.enc` — l'implémentation réelle — et distinguer explicitement
  ce chemin de l'espace `x/<id>/…` de §08.2:232-238. Coût texte faible, coût code
  nul. Rend §02.3 conforme au code.
- **(b)** Déplacer le vault sous `x/<id>/` et généraliser `is_header_file`. Coût
  lourd : toutes les clés de store changent, donc la carte `files` du manifeste
  change, donc **toutes les éditions signées** doivent être republiées.
- Dans **les deux cas**, rendre `is_header_file` indépendant du préfixe `e/` :
  c'est la partie de la correction qui ferme l'angle mort I3 quelle que soit
  l'option retenue, et elle est bon marché isolément.
- Ce que cela casserait : (a) rien au niveau du wire ; (b) toute la chaîne
  d'éditions. Le durcissement d'`is_header_file` seul peut invalider une édition
  historique qui aurait épinglé un objet en `.../header.json` hors de `e/` sans
  ligne owner.
- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle.**

---

## SC-12 — Portée effective d'une entrée de périmètre `revoke` non scopée

**Famille 1 — Contradiction. Classe : `LES DEUX IMPLÉMENTÉS`. Invariant touché : I4.
Sévérité : élevée** (justifiée en (3)).

> **Levée d'embargo.** Retenu à l'identifiant, au titre neutre et aux trois
> sections normatives du **2026-08-04T07:40Z** au **2026-08-04T13:00Z**, sous la
> « condition 9 » de la barrière de divulgation, telle que
> `features/.agents/orchestrator/BLOCKED.md` la nomme : dépôt public, faiblesse
> exploitable, aucun correctif disponible. **Levé par le propriétaire (Mathieu
> Colla) le 2026-08-04**, décision consignée dans
> `features/.agents/orchestrator/BLOCKED.md` § « Résolues ».
>
> *(`BLOCKED.md` rattache cette « condition 9 » à une section « Blocking
> conditions » de `PROCESS.md` qui **n'existe pas** à `223924e`. Ce n'est pas un
> constat de cette passe : c'est **`CHDR-040`**, `OPEN`, P2,
> `docs/audits/features/c-headers.md:2385-2397`, qui vise le train et non une
> feature, et dont le point 2 nomme précisément « la liste numérotée des
> conditions de blocage, 1 à 10 ». Troisième observation indépendante du même
> défaut ; elle est versée à cet identifiant et n'en ouvre pas un nouveau.)*
>
> L'énoncé ci-dessous n'est **pas** celui qui avait été remis hors dépôt. Le
> fichier `/root/work/EMBARGO-SC-12.md` n'existait plus au moment de la levée :
> l'effacement silencieux du clone local l'a détruit. Le constat est donc
> **re-dérivé depuis `spec/` et depuis le code à `223924e`**, et non restitué de
> mémoire. Deux éléments que le résumé survivant de `BLOCKED.md` attribuait à ce
> constat **ne survivent pas** à la re-dérivation ; ils sont signalés sur place en
> (2a) et en (4).
>
> **Pourquoi la re-dérivation était la bonne méthode, et ce n'est pas une
> assertion — c'est montrable.** Le résumé survivant défendait la non-correction
> de ce constat par un argument unique : durcir le contrôle invaliderait
> rétroactivement des entrées déjà publiées, et `spec/00-overview.md` §0.4
> n'autorise qu'**un seul** durcissement rétroactif dans cette série, déjà dépensé
> sur I3. Cette phrase de §0.4 **n'existait plus** au moment où l'argument était
> encore invoqué. Elle a été supprimée par le commit `c8557f4`, « spec: I3 binds
> profiles, not time — and stop costing backward compatibility », le
> 2026-08-04T08:01Z — vingt-quatre minutes après le commit de cette passe
> (`d3ce85f`, 07:37Z).
>
> **`c8557f4` est le lot de spécification de l'orchestrateur lui-même**, appliquant
> la décision du propriétaire du 2026-08-04 qui a fermé la condition de blocage 1.
> L'orchestrateur en assume la paternité et la conséquence : il a retiré cette
> phrase délibérément, dans le cadre de la correction de §0.4, **sans voir qu'un
> autre constat, dans un autre document, faisait reposer toute sa sévérité
> dessus**. Le constat est alors resté sous embargo cinq heures, défendu par une
> phrase qui n'existait plus.
>
> Personne ne l'a vu jusqu'à ce que ce constat soit re-dérivé depuis les sources
> au lieu d'être restitué de mémoire. Si le fichier hors dépôt avait survécu et
> avait simplement été recollé, `SC-12` serait publié aujourd'hui avec un argument
> mort à l'intérieur. C'est la démonstration, et non la promesse, qu'une
> correction dans un document peut retirer silencieusement la prémisse porteuse
> d'un constat retenu dans un autre — et qu'une barrière de divulgation qui garde
> ses énoncés hors du dépôt les soustrait précisément aux relectures qui
> l'auraient attrapé.

### (1) Les deux côtés, verbatim

`spec/00-overview.md:41-45` (I4) — **lu jusqu'au point**, la dernière phrase
portant tout le cas délégué :

> 4. **I4 — Authority follows issuance.** Only the issuer of a mandate (or an ancestor
>    in its chain, transitively up to the owner) may revoke it or remove its lines.
>    Verifiable from certificates alone. A `revoke` perimeter entry (§04.2, §06.7)
>    delegates the *certificate* half of this authority — never the key half — within
>    attenuation.

`spec/04-mandates.md:183-186` (§04.2, règle `covers` normative) — les deux
phrases, lues jusqu'à leurs points respectifs :

> A `revoke` entry conveys no key and no
> read: only the authority to publish revocation entries for mandates whose perimeter
> it covers; attenuation applies (§06.7). A bare `revoke` covers the issuer's own
> revocable scope.

`spec/06-revocation.md:121-130` (§06.7) — la clause d'atténuation **et** la
clause de modèle de menace qui la ferme, lues jusqu'au point final :

> A mandate MAY carry a `revoke` perimeter entry (§04.2) while holding **no content key
> at all** (no header line anywhere). Its bearer — a daemon, a Lambda, a phone app —
> can publish revocation entries for any mandate whose perimeter its `revoke` scope
> covers (attenuation applies: it can only be granted `revoke` over what its issuer
> could itself revoke), cutting the revoked party's *actions* instantly at every
> honoring verifier. It can neither read a byte nor rotate a lock. Rotation — the
> future-read cut — is then executed by a manager-holder on notification, or as lazy
> hygiene (§6.8). Compromising the watchdog exposes no content; the worst abuse is a
> revocation DoS, bounded to its perimeter, attributable (signed), and repaired at one
> line per victim (re-grant, §03.3).

et `spec/06-revocation.md:79-84`, qui fait de cette portée une **règle de
validité d'entrée** et non une recommandation :

> - the owner (owner-signed entry), or
> - the revoked mandate's **issuer** (revoker leaf grantee key == `issued_by`), or
> - a **transitive ancestor** (the revoker's leaf mandate id appears in the
>   revoked mandate's parent chain), or
> - a **watchdog** whose `revoke` perimeter covers the revoked mandate's
>   perimeter (§6.7, attenuation applies).

La grammaire de §04.2 admet enfin les deux formes, `spec/04-mandates.md:113` :

> ```
>   | "revoke" [ "." <zone> [ "#" <selector> ] ]  revocation right, certificate half only (§06.7)
> ```

**Les deux côtés.** §04.2:183-185 et §06.7:123-124 énoncent **un test de
périmètre** : le revocateur peut couper les mandats « whose perimeter it covers »,
c'est-à-dire un test de treillis entre deux périmètres. §04.2:185-186 énonce, pour
la forme **non scopée**, **un test d'ascendance** : « the issuer's own revocable
scope », or « the issuer's revocable scope » n'est défini nulle part ailleurs que
par I4 (`00-overview.md:41-42`) — les mandats que l'émetteur a émis, et leurs
descendants. Un `revoke` nu n'a pas de périmètre à soumettre au premier test : le
test de treillis est **vide** sur lui, et le test d'ascendance n'est pas exprimable
depuis un périmètre. Les deux tests sont dans `spec/`, ils ne donnent pas la même
réponse sur la forme nue, et — c'est le point — **les deux sont dans le code**.

### (2) Ce que le code fait de chaque côté

Le contrôle d'autorité est unique et vit dans un fichier de 131 lignes :
`rust/crates/aithos-core/src/revocation.rs`, `check_revoke_authority`
(`:57-103`). Il a trois branches d'acceptation : propriétaire (`:67-69`),
watchdog (`:74-81`), puis émetteur (`:83-86`) et ancêtre transitif (`:88-97`).
**La branche watchdog est testée en premier et renvoie avant les deux autres.**
Elle contient deux défauts distincts. Le second est celui que l'embargo protégeait
et il est énoncé en (2b) ; **le premier est plus large, et il n'était dans aucun
dossier** — il vient en tête pour cette raison.

#### (2a) Le trou le plus large : quatre variantes de périmètre ne sont jamais testées

`features/.agents/orchestrator/BLOCKED.md` résume ce constat par « the mitigation
is correctly coded for the scoped case, and the bare case short-circuits it ».
La seconde moitié est exacte. **La première est fausse**, et c'est la trouvaille
principale de la re-dérivation : la forme **scopée** n'est pas bornée non plus.

`rust/crates/aithos-core/src/revocation.rs:111-118` filtre les entrées du
périmètre de la cible avant de les soumettre au treillis :

```rust
for te in &target_perimeter {
    let ethos_like = matches!(
        te,
        PerimeterEntry::Ethos { .. } | PerimeterEntry::Act { .. }
    );
    if !ethos_like {
        continue;
    }
```

Or `PerimeterEntry` a six variantes (`rust/crates/aithos-core/src/mandate.rs:72-113`) :
`Ethos`, `EthosId`, `Act`, `Gamma`, `Issue`, `Revoke`. Quatre sont donc
**sautées**. Un mandat cible dont le périmètre ne contient aucune entrée `Ethos`
ni `Act` traverse la boucle sans qu'aucun test s'exécute, et `revoke_covers`
renvoie `Ok(true)` **par vacuité** — pour n'importe quel revocateur portant
n'importe quelle entrée `revoke`, si étroite soit-elle, puisque `has_revoke_right`
(`:76-78`) accepte `Revoke { .. }` sans regarder sa portée.

Ce n'est pas une hypothèse : `write.circle#id=<sid>` se parse en `EthosId`
(`mandate.rs:199-203`, `#id=` ne compose avec rien, D1 — `mandate.rs:200-201`), et
le vecteur normatif `vectors/cb2-mandate-contracts.json:294-295` gèle deux mandats
dont les périmètres sont exactement `["read.circle#id=…","issue#depth=1"]` et
`["read.circle#id=…"]` — aucune entrée `Ethos`, aucune entrée `Act`. Un watchdog
scopé sur `revoke.public#dir=<autre>` les révoque tous les deux.

La borne réelle du modèle de menace de §06.7 est donc : **le périmètre de la cible
doit contenir au moins une entrée `Ethos` ou `Act` pour que l'atténuation
s'applique du tout**. Cette phrase n'est écrite nulle part dans `spec/`.

**Pourquoi cette moitié compte plus que l'autre.** (2b) suppose que le
propriétaire a émis un `revoke` nu. (2a) ne suppose rien : il suffit d'un watchdog
scopé — le cas que §06.7 présente comme le cas sûr, celui que le seul scénario
Gherkin de watchdog exerce — et d'une cible dont le périmètre est gravé tel quel
dans un vecteur du dépôt. Ce défaut est strictement pire que celui qui était sous
embargo, et il a été trouvé par l'acte de vérifier, pas par l'acte de se souvenir.

#### (2b) La portée nue court-circuite l'atténuation

- **Côté treillis (§06.7), codé au treillis pour la forme scopée** — correctement
  quant au treillis lui-même, sous la réserve de (2a) sur les entrées qui n'y
  arrivent jamais. `rust/crates/aithos-core/src/revocation.rs:119-125`, dans
  `revoke_covers` :
  ```rust
  let covered = revoker_perimeter.iter().any(|re| match re {
      // A bare `revoke` covers the issuer's whole revocable scope;
      // a scoped `revoke.<zone>#…` covers by the same lattice as reads.
      PerimeterEntry::Revoke { scope: None } => true,
      PerimeterEntry::Revoke { scope: Some(s) } => covers(s, te),
      _ => false,
  });
  ```
  La ligne `:123` est l'atténuation de §06.7 écrite littéralement : la portée
  déclarée du revocateur doit couvrir, au même treillis que les lectures
  (`rust/crates/aithos-core/src/mandate.rs:342-457`, `PerimeterEntry::covers`),
  chaque entrée du périmètre de la cible.

- **Côté portée nue, qui court-circuite le premier.** La ligne `:122` renvoie
  `true` **inconditionnellement**, sans regarder ni la cible, ni l'émetteur du
  `revoke` nu, ni aucune ascendance. `revoke_covers` renvoie donc `Ok(true)` pour
  **toute** cible, `check_revoke_authority` renvoie `Ok(())` en `:79-81`, et les
  branches émetteur/ancêtre — les seules qui implémentent la phrase I4 — ne sont
  jamais atteintes. Le commentaire de `:120-121` et celui de l'énumération
  (`rust/crates/aithos-core/src/mandate.rs:109-112`, « `None` scope = the issuer's
  whole revocable reach ») énoncent tous deux la borne de §04.2:185 ; le code
  au-dessous ne l'implémente pas, parce que `revoke_covers` ne reçoit que deux
  périmètres (`revocation.rs:108`) et jamais l'identité de l'émetteur — alors que
  `check_revoke_authority`, lui, tient les deux chaînes complètes (`:58-59`) et
  pourrait la calculer.

**Conséquence, énoncée sans détour.** Le porteur d'un mandat dont le périmètre
contient l'entrée `revoke` nue peut publier une entrée `revoke` valide contre
**n'importe quel** mandat du sujet : un délégué émis directement par le
propriétaire, un frère sans lien avec lui, son propre émetteur, son propre
ancêtre. La borne annoncée par §06.7:128-130 — « bounded to its perimeter » — est
vide sur la forme nue, puisque cette forme n'a pas de périmètre à borner.

**Les deux côtés sont atteints par un vérificateur, pas seulement par un
producteur.** `grep -rn "check_revoke_authority" --include=*.rs .` (périmètre :
dépôt entier, couche code) renvoie six lignes : la définition
(`rust/crates/aithos-core/src/revocation.rs:57`), deux `use`
(`rust/crates/aithos-bundle/src/revoke.rs:19`,
`rust/crates/aithos-core/src/gamma_replay.rs:18`) et **trois** sites d'appel, pas
un de plus :
`rust/crates/aithos-bundle/src/revoke.rs:65` (`log_revoke_as`, production
d'entrée), `rust/crates/aithos-bundle/src/revoke.rs:105` (`active_revocations`,
reconstruction de l'ensemble actif §06.5) et
`rust/crates/aithos-core/src/gamma_replay.rs:355` — dans `verify_semantics`, donc
dans `GammaReplayState::admit`, que `rust/crates/aithos-bundle/src/log.rs:860-862`
exécute pour **chaque** entrée dans `gamma_verify`, « Full offline log
verification ». L'entrée forgée n'est donc pas seulement acceptée à l'écriture :
elle est ratifiée par le rejeu froid sans clé.

### (3) Classe et sévérité

`LES DEUX IMPLÉMENTÉS`. Les deux comportements coexistent dans une seule fonction
et sur une seule ligne d'écart : `:123` implémente l'atténuation de §06.7, `:122`
l'annule. Ce n'est pas une omission — la ligne `:122` est délibérée, commentée, et
elle transcrit une phrase de `spec/` (§04.2:185) dont elle donne la lecture la
plus large. Arbitrer veut dire supprimer un des deux comportements, et l'un des
deux est adossé à une phrase normative.

**Sévérité : élevée.** Trois raisons, et la troisième est celle qui la fait
monter. D'abord la portée : la coupure atteint la totalité du graphe de délégation
du sujet, y compris les délégués directs du propriétaire, depuis un mandat qui ne
détient **aucune** clé — c'est exactement le profil que §06.7 décrit comme le moins
dangereux à confier. Ensuite la surface : le rejeu froid `gamma_verify` ratifie
l'entrée, donc la coupure est opposable à tout vérificateur honnête, pas seulement
au producteur qui l'a écrite. Enfin, et c'est le point (2a), la borne annoncée
ne tient pas non plus pour la forme scopée : le propriétaire n'a pas besoin
d'avoir commis l'imprudence d'émettre un `revoke` nu pour que la promesse
« bounded to its perimeter » soit fausse.

**Ce qui la retient de monter plus haut.** La forme nue ne se délègue pas vers le
bas : `rust/crates/aithos-core/src/mandate.rs:448-454` refuse `(Some(_), None)` (`:452`),
donc un revocateur scopé ne peut pas émettre un enfant portant un `revoke` nu, et
la règle de containment de périmètre (`mandate.rs:1128-1141`, §05.3 règle 1)
l'applique à chaque lien. Une entrée `revoke` nue ne peut donc apparaître que dans
un mandat racine signé par le propriétaire, ou sous une chaîne de `revoke` nus
issue d'une telle racine. C'est une borne réelle, mais c'est une borne sur *ce qui
peut être accordé*, jamais sur *ce qui peut être révoqué* — et c'est précisément
la confusion que §04.2:185 installe.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Deux options exclusives, à trancher au gate humain. La différence entre elles est
un choix de protocole, pas d'implémentation.

- **(a) Aligner le code sur I4.** Donner à `revoke_covers` le paramètre qui lui
  manque — la chaîne du revocateur, que `check_revoke_authority` tient déjà
  (`revocation.rs:58`) — et traiter `Revoke { scope: None }` non plus comme `true`
  mais comme « l'émetteur de cette entrée est un ancêtre de la cible », c'est-à-dire
  la règle de `:88-97` appliquée à `revoker_chain[len-2]` au lieu de
  `revoker_chain[len-1]`. Corriger dans le même geste le filtre `:111-118` pour
  qu'il couvre les six variantes de `PerimeterEntry`, ou pour qu'il **échoue
  fermé** sur toute variante qu'il ne sait pas comparer. Coût : moyen, entièrement
  dans `aithos-core/src/revocation.rs` ; aucun octet signé ne change, le wire ne
  bouge pas.
- **(b) Aligner la spec sur le code.** Retirer la phrase §04.2:185-186 et réécrire
  §06.7:128-130 pour dire ce qui est vrai : un `revoke` nu confère l'autorité de
  révocation universelle sur le sujet, et il ne doit être émis qu'au niveau
  racine par le propriétaire, comme un rôle de rupture d'urgence. Coût texte
  faible ; mais cela publie une escalade de privilège dans le modèle de menace au
  lieu de la corriger, et cela laisse le trou de vacuité de (2a) intact —
  celui-là n'a **aucune** phrase de `spec/` pour l'adosser et doit être fermé dans
  les deux options.

**Ce que cela casserait.** Rien dans le dépôt. La recherche est explicite :
aucun fichier de `vectors/` ne contient d'entrée de périmètre commençant par
`revoke` (parcours programmatique de chaque clé `perimeter` de chaque
`vectors/*.json`, couche corpus de données) ; les seules chaînes `revoke` /
`revoke.circle#id=…` de `vectors/cb2-mandate-contracts.json:690-697` sont des cas
d'aller-retour de parsing, pas des périmètres opposables. Les 73 entrées
`kind:"revoke"` déléguées de `vectors/cb2-delegated-counts.json` sont des fixtures
de comptage : ce vecteur ne contient **aucun** champ `perimeter` (0 occurrence).
Le seul scénario Gherkin de watchdog, `features/g-revocation.feature:51-55` et
`features/k-integration.feature:147-155`, utilise une portée **scopée**
(`rust/crates/aithos-bundle/tests/cucumber.rs:15343-15351`,
`revoke.circle#dir=…`) contre une cible portant une entrée `Ethos` : il reste vert
sous l'option (a).

Cette dernière affirmation repose désormais sur une mesure et non sur une lecture.
La feature `@g-revocation` est **verte** à `223924e` — `ev-63e018d1`, 1 feature /
9 règles / **26 scénarios** / 116 étapes — et le vecteur d'autorité §06.4 l'est
aussi — `ev-b8cee044`, `g1_revocation`, 2 tests passés. La ligne de base de comparaison est
l'espace de travail entier, vert à `223924e` : **`ev-cb4ff302`**, 18 features /
114 règles / 836 scénarios / 3 577 étapes, plus l'ensemble des tests unitaires.
Tout RED écrit pour le critère de clôture ci-dessous est donc attribuable depuis
cette ligne. Ces transcriptions établissent l'**état vert de départ** ; **aucune
ne démontre le défaut lui-même**, qui n'a pas de test — c'est exactement ce que le
critère de clôture demande d'écrire, et le drapeau de lecture reste levé sur lui.

**Le motif de blocage enregistré ne survit pas.** `BLOCKED.md` retient contre le
durcissement qu'il « would retroactively invalidate entries already published »,
et que `spec/00-overview.md` §0.4 n'autorise qu'un seul durcissement rétroactif
dans cette série, déjà dépensé sur I3. **Cette phrase n'existe plus dans
`spec/`.** Elle a été supprimée par `c8557f4` (« spec: I3 binds profiles, not
time », 2026-08-04T08:01Z), vingt-quatre minutes après le commit de cette passe
(`d3ce85f`, 07:37Z) : `grep -rni "retroactiv" spec/` ne renvoie aujourd'hui
qu'une occurrence, `spec/07-gamma.md:156`, sans rapport. Le budget d'un
durcissement n'existe donc pas, et l'argument n'a plus d'ancrage normatif.

Reste la question de fond, qu'il faut séparer proprement, parce que l'argument
visait deux choses à la fois :

- **Au niveau du protocole, dans un déploiement hypothétique, il tient encore.**
  Le rejeu gamma n'est pas le rejeu d'édition : `GammaReplayState::admit` repart
  du genèse et re-soumet chaque entrée historique à `check_revoke_authority`
  (`gamma_replay.rs:1-6`, « Pure, prefix-sensitive semantic replay for historical
  Gamma entries »). Une entrée `revoke` acceptée hier serait rejetée demain, et
  `spec/00-overview.md:82` interdit la réparation : « Historical manifests and
  entries are never rewritten or assigned synthetic references. » Le journal
  entier deviendrait invérifiable. C'est un vrai coût, et il faudra le porter le
  jour où un journal existera. La levée d'ambiguïté du 2026-08-04 sur §0.4 ne le
  couvre pas : elle porte sur les **éditions superseded**, que le vérificateur ne
  reparcourt pas ; les entrées gamma, elles, sont reparcourues.
- **Au niveau de ce dépôt, il est nul aujourd'hui — et c'est une politique, pas
  seulement un fait.** `features/AGENTS.md` § *Project stage* (`:5-34`) constate
  que rien n'est déployé, qu'aucune édition n'a été publiée et qu'aucun détenteur
  n'existe, puis en tire une **règle** que tout rôle de feature lit par le routage
  obligatoire : « Do not weigh backward compatibility […] Do not soften a
  correction to spare the past. » Un rôle n'a donc pas seulement le droit
  d'ignorer ce coût, il a l'instruction de ne pas le compter. Le paragraphe
  précédent l'établit en outre sur les artefacts eux-mêmes : aucun vecteur, aucun
  scénario ne dépend de la branche `:122`, et `ev-cb4ff302` mesure l'espace de
  travail vert sans en dépendre. Il n'y a donc **rien** à invalider
  rétroactivement, et le coût de l'option (a) est celui de son diff.

  **Cette réponse est écrite pour expirer, et il faut la relire plutôt que
  l'hériter.** La même section (`features/AGENTS.md:30-34`) cesse d'être vraie « the
  day a first edition is published outside this repository, or the crate leaves
  `alpha` », et charge celui qui franchit l'une des deux bornes de la supprimer
  dans le même changement. Le jour où elle disparaît, le niveau protocole ci-dessus
  redevient le niveau applicable et la question doit être **reposée**, pas déduite
  de ce paragraphe. Un rôle qui trouverait la section encore présente après l'une
  des deux bornes doit le signaler au lieu d'obéir : elle le dit elle-même.

L'argument enregistré était donc un argument de protocole, écrit au futur, appliqué
par erreur au présent d'un dépôt vide. C'est ce qui a maintenu la rétention plus
longtemps qu'elle ne le méritait.

**Critère de clôture** — attribuable depuis `ev-cb4ff302` (espace de travail vert
à `223924e`), `ev-63e018d1` (`@g-revocation` vert, 26 scénarios) et `ev-b8cee044`
(`g1_revocation` vert, 2 tests). Un test RED démontre qu'un mandat portant l'entrée
`revoke` nue, dont l'émetteur n'est ni le propriétaire ni un ancêtre de la cible,
voit son entrée `revoke` refusée par `check_revoke_authority`, par
`Bundle::active_revocations` et par `Bundle::gamma_verify` — là où elle est
aujourd'hui acceptée par les trois ; et un second test RED démontre le même refus
contre une cible dont le périmètre ne contient que des entrées `EthosId`,
`Gamma`, `Issue` ou `Revoke`, pour un revocateur scopé qui ne la couvre pas. Le
scénario `features/g-revocation.feature:51-55` reste vert sans modification — au
compte mesuré de `ev-63e018d1`, 1 feature / 9 règles / 26 scénarios / 116 étapes.

- **La faisabilité de ces arbitrages n'a pas été vérifiée par ce rôle.** Quatre
  gates ont été exécutés par l'orchestrateur pour ce constat et pour SC-05 —
  `ev-cb4ff302`, `ev-fafd51d8`, `ev-63e018d1`, `ev-b8cee044` — et ils établissent
  une **ligne de base verte**, rien de plus. Aucun d'eux n'exerce ni la branche
  `:122` ni le filtre `:111-118` ; **aucun ne démontre le défaut**. Les
  affirmations de (2a) et (2b) restent donc des conclusions de **lecture**, et le
  drapeau posé sur elles n'est pas levé. Il le sera par les deux RED du critère de
  clôture, et pas avant.

---

## SC-13 — « There is no wire verb `create` » face aux tables K1.2

**Famille 1 — Contradiction. Classe : `TEXTUELLE`.**

### (1) Les deux côtés, verbatim

`spec/04-mandates.md:147-153` :

> Verb lattice (normative): `read ⊑ edit ⊑ append ⊑ write`,
> `read ⊑ delete ⊑ write`; `delete` is otherwise incomparable with `edit` and
> `append`. Operationally, create requires `append` or `write`; editing an existing
> object accepts `edit`, `append`, or `write`; deletion accepts `delete` or `write`
> and always includes read authority; `write` is full CRUD. There is no wire verb
> `create`.

`spec/04-mandates.md:708-709` :

> `verb` is exactly `create`, `edit`, `delete`, or `redact`; `zone` is exactly
> `public`, `circle`, or `self`; `sid` is the target section's canonical SID.

et `spec/04-mandates.md:792-793` :

> For the structural family, `verb` is exactly `create`, `rename`, `delete`, or
> `move`.

`create` est donc bien un littéral de wire — dans le document de faits
d'opération, dont le JCS entre dans un digest signé
(`spec/04-mandates.md:519-522`).

### (2) Ce que le code fait de chaque côté

Les deux couches existent et sont distinctes, sans mécanisme de traduction
documenté dans `spec/`.

- Côté périmètre, `create` n'est pas un verbe : le registre de verbes du crate
  `aithos-core` et la fonction de containment `covers()` — celle que §04.2:132
  nomme déjà — ne connaissent que `read|edit|append|delete|write`.

  > **Intervalle de lignes restauré le 2026-08-04.** Ce point portait la mention
  > « le `fichier:ligne` n'est pas donné : le module concerné héberge aussi l'un
  > des deux sites de SC-12, retenu pour divulgation, et un intervalle de lignes
  > l'encadrerait ». La relecture confirme que ce retrait n'avait **aucun autre
  > motif** que la protection de SC-12, dont l'embargo est levé. Le module est
  > `rust/crates/aithos-core/src/mandate.rs` : `enum Verb` (`:25-31`),
  > `Verb::parse` (`:34-43`, `other => return Err(…"unknown verb"…)`), le treillis
  > `Verb::covers` (`:58-67`) et `PerimeterEntry::covers` (`:342-457`). Aucune de
  > ces lignes ne connaît `create`. Rien d'autre de ce constat n'est modifié.
- Côté faits d'opération, `create` est un littéral obligatoire, dans
  `rust/crates/aithos-core/src/operation.rs`, avec ses vecteurs
  `vectors/cb2-operation-facts-mutation.json` et
  `vectors/cb2-operation-facts-structural.json`.

### (3) Classe

`TEXTUELLE`. Aucun comportement à supprimer : la phrase de §04.2 est correcte
dans son propre registre (la grammaire de périmètre) et fausse hors de lui. Le
défaut est que §04.2 emploie l'expression « wire verb » sans la restreindre, alors
que §04.5.1 a gravé des littéraux `verb` dans des octets signés. Sévérité faible ;
retenue parce que c'est exactement le genre de phrase qu'une seconde
implémentation lirait comme une interdiction globale.

### (4) Arbitrage proposé — `PROPOSÉ — NON IMPLÉMENTÉ`

Restreindre la phrase : « There is no `create` verb in the **perimeter** grammar;
the K1.2 operation-facts families use their own closed `verb` registries
(§4.5.1). »

- Coût estimé : faible. Une phrase de `spec/04-mandates.md:152-153`. Aucun octet
  signé.
- Ce que cela casserait : rien.
- **La faisabilité de cet arbitrage n'a pas été vérifiée par ce rôle.**

---

## `follow_ups` proposé pour `features/.agents/orchestrator/QUEUE.yaml`

> Bloc proposé, à insérer sous la clé `follow_ups`. Identifiants stables préfixés
> `SPEC-CONS-`. Aucun de ces suivis n'est implémenté ; chacun requiert son propre
> cycle de correction, qui vérifiera la faisabilité de l'arbitrage.

```yaml
follow_ups:
  - id: SPEC-CONS-01-GAMMA-LAYOUT
    title: "Emplacement canonique du journal Gamma : §02.3 vs §07.1"
    family: contradiction
    class: les-deux-implementes
    invariant: null
    spec_refs: ["spec/02-content-tree.md:72", "spec/07-gamma.md:9", "spec/01-identity-and-keys.md:96"]
    code_refs: ["rust/crates/aithos-bundle/src/log.rs:29", "rust/crates/aithos-bundle/src/lib.rs:155", "rust/crates/aithos-cli/src/cmd/init.rs:70"]
    proposed_arbitration: "Aligner §02.3 sur la forme segmentee; redefinir `revocations` comme prefixe."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["vectors/a2-did.json", "octets signes de tout did.json publie"]
    gate_required: true

  - id: SPEC-CONS-02-AAD-PURPOSES
    title: "Inventaire des purposes AAD de §00.3 (gamma-payload, vault, header_path)"
    family: contradiction
    class: textuelle
    invariant: null
    spec_refs: ["spec/00-overview.md:62", "spec/07-gamma.md:124", "spec/03-headers.md:32"]
    code_refs: ["rust/crates/aithos-core/src/gamma.rs:21", "rust/crates/aithos-bundle/src/vault.rs:146", "rust/crates/aithos-core/src/header.rs:261"]
    proposed_arbitration: "Reecrire la phrase §00.3 : blob, tagwrap, gamma-body; header-line lie au node."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: []
    gate_required: false

  - id: SPEC-CONS-03-GAMMA-KIND-STRUCTURAL
    title: "Aucun kind Gamma pour une mutation structurelle ou vault-config"
    family: lettre-morte
    class: aucun
    invariant: I5
    spec_refs: ["spec/00-overview.md:46", "spec/07-gamma.md:326", "spec/04-mandates.md:792", "spec/04-mandates.md:1818"]
    code_refs: ["rust/crates/aithos-core/src/gamma.rs:26", "rust/crates/aithos-bundle/src/structure.rs:889", "rust/crates/aithos-bundle/src/vault.rs:276"]
    proposed_arbitration: "Ouvrir le registre sous un bump `v`, OU specifier payload.structural tel quel."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["racines de segment H2 (§07.10) : aucune reecriture possible, cohabitation obligatoire"]
    gate_required: true

  - id: SPEC-CONS-04-ETHOS-READ-VAULT
    title: "ethos.read emis hors mandat log_reads et en payload clair (vault-config)"
    family: contradiction
    class: les-deux-implementes
    invariant: I5
    spec_refs: ["spec/07-gamma.md:342", "spec/07-gamma.md:334", "spec/04-mandates.md:1805", "spec/04-mandates.md:1818"]
    code_refs: ["rust/crates/aithos-bundle/src/vault.rs:249", "rust/crates/aithos-bundle/src/vault.rs:196", "rust/crates/aithos-core/src/constraints.rs:941"]
    proposed_arbitration: "(a) fail-closed sur la lecture de config, OU (b) lever la restriction pour le domaine vault-config."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["(a) casse le chemin de lecture de config du gateway"]
    gate_required: true

  - id: SPEC-CONS-05-MAX-SESSIONS
    title: "max_sessions : tier V en §04.4 contre fail-closed en §04.7/§04.13"
    family: contradiction
    class: "textuelle (moitie spec) + aucun (moitie code)"
    invariant: null
    spec_refs: ["spec/04-mandates.md:238", "spec/04-mandates.md:229", "spec/04-mandates.md:1347", "spec/04-mandates.md:1789"]
    code_refs: ["rust/crates/aithos-core/src/constraints.rs:923", "rust/crates/aithos-core/src/constraints.rs:1293", "rust/crates/aithos-core/src/constraints.rs:1341", "rust/crates/aithos-owner/src/lib.rs:812", "vectors/cb2-mandate-contracts.json:68"]
    proposed_arbitration: >-
      Moitie spec : retirer le tier V de §04.4 ou l'aligner sur la ligne reservee
      de §04.13. Moitie code, deux options exclusives : (a) sortir max_sessions de
      la liste des cles connues et echouer ferme au lien de delegation, OU (b)
      definir le wire de cycle de vie de session de §04.7 et brancher
      verify_max_sessions sur un ensemble reconstruit depuis les fichiers.
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["(a) rust/crates/aithos-owner/src/lib.rs:812", "(a) vectors/cb2-mandate-contracts.json", "(a) vectors/cb14-delegated-session-chain.json", "(a) vectors/cb15-external-delegated-grant.json", "(a) rust/crates/aithos-core/tests/cb5_evidence_contracts.rs:112-131", "(b) nouveau construit signe, donc nouveau profil §00.4"]
    gate_required: true
    disclosure: lifted-2026-08-04
    baseline_evidence: ["ev-cb4ff302", "ev-fafd51d8"]
    disclosure_note: >-
      Bord code retenu du 2026-08-04T07:40Z au 2026-08-04T13:00Z sous la
      condition de blocage 9, puis publie en entier sur decision du proprietaire.
      Re-derive depuis spec/ et depuis le code a 223924e, non restitue : le
      fichier hors depot avait ete detruit. L'ecart est permissif : le code
      accepte et consomme un mandat portant max_sessions, la ou §04.13 exige un
      fail-closed et §04.4 promet un tier V. Classe de la moitie code : AUCUN.

  - id: SPEC-CONS-06-ROTATION-EXACTLY-N
    title: "Le repli exactly-N de §03.4 est rejete par check_rotation"
    family: inatteignable
    class: inatteignable
    invariant: I3
    spec_refs: ["spec/03-headers.md:87", "spec/03-headers.md:108"]
    code_refs: ["rust/crates/aithos-core/src/header.rs:347", "rust/crates/aithos-bundle/src/revoke.rs:217"]
    proposed_arbitration: "Passer les kids du header de P a check_rotation, OU supprimer le repli de §03.4."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["signature publique de Header::check_rotation", "vectors/g2-rotation.json (extension normative §09.2)"]
    gate_required: true

  - id: SPEC-CONS-07-REVOCATION-BY-OMISSION
    title: "La revocation par omission (§05.5) n'est verifiee nulle part"
    family: lettre-morte
    class: aucun
    invariant: I4
    spec_refs: ["spec/05-delegation.md:101", "spec/10-threat-model.md:117"]
    code_refs: ["rust/crates/aithos-core/src/header.rs:347", "rust/crates/aithos-bundle/src/bundle.rs:318"]
    proposed_arbitration: "Etendre check_rotation avec la chaine du signataire et l'appeler depuis verify_pinned_headers."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["second durcissement retroactif : §00.4:88 n'en autorise qu'un (I3)"]
    gate_required: true

  - id: SPEC-CONS-08-UPLINK-WRAP-AUTHORITY
    title: "L'autorite de l'auteur d'un up-link wrap n'est pas determinable depuis le wire"
    family: inatteignable
    class: inatteignable
    invariant: I4
    spec_refs: ["spec/03-headers.md:112", "spec/10-threat-model.md:118", "spec/09-cli-and-conformance.md:55"]
    code_refs: ["rust/crates/aithos-core/src/header.rs:406", "rust/crates/aithos-bundle/tests/cucumber.rs:15298"]
    proposed_arbitration: "(a) reformuler §03.4 en propriete physique, OU (b) ajouter author+sig au Wrap."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["(b) casse toutes les racines d'etat signees : §02.10:594 hache BLAKE3(JCS(wrap))"]
    gate_required: true

  - id: SPEC-CONS-09-TAG-REPAIR-PASS
    title: "La passe de reparation de wraps de tag (§02.2) n'existe pas"
    family: lettre-morte
    class: aucun
    invariant: null
    spec_refs: ["spec/02-content-tree.md:52", "spec/09-cli-and-conformance.md:60"]
    code_refs: ["rust/crates/aithos-bundle/src/structure.rs:861"]
    proposed_arbitration: "(a) retirer la clause et l'exigence de vecteur, OU (b) implementer la passe."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["(b) exige des cles de contenu, hors du profil recursive-maintenance §00.5"]
    gate_required: true

  - id: SPEC-CONS-10-EPOCH-SIGNER-SENTENCE
    title: "§01.4 : 'It is signed by root_sign' suit le rejet explicite de #root"
    family: contradiction
    class: textuelle
    invariant: null
    spec_refs: ["spec/01-identity-and-keys.md:116", "spec/01-identity-and-keys.md:36"]
    code_refs: ["rust/crates/aithos-core/src/did.rs:20", "rust/crates/aithos-core/src/did.rs:239"]
    proposed_arbitration: "Deplacer les deux phrases dans le paragraphe du document DID (apres :103)."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: []
    gate_required: false

  - id: SPEC-CONS-11-VAULT-PATH-I3-BLINDSPOT
    title: "Emplacement canonique du node vault et angle mort I3 de is_header_file"
    family: inatteignable
    class: inatteignable
    invariant: I3
    spec_refs: ["spec/02-content-tree.md:70", "spec/02-content-tree.md:22", "spec/03-headers.md:8", "spec/09-cli-and-conformance.md:99"]
    code_refs: ["rust/crates/aithos-bundle/src/vault.rs:55", "rust/crates/aithos-bundle/src/lib.rs:204", "rust/crates/aithos-bundle/src/bundle.rs:291"]
    proposed_arbitration: "(a) graver e/x/<id>/ dans §02.3, OU (b) deplacer le vault sous x/<id>/. Dans les deux cas, rendre is_header_file independant du prefixe e/."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["(b) casse toute la chaine d'editions signees"]
    gate_required: true

  - id: SPEC-CONS-12-REVOKE-SCOPE
    title: "Portee effective d'une entree de perimetre revoke non scopee"
    family: contradiction
    class: les-deux-implementes
    invariant: I4
    spec_refs: ["spec/00-overview.md:41", "spec/04-mandates.md:183", "spec/06-revocation.md:79", "spec/06-revocation.md:121"]
    code_refs: ["rust/crates/aithos-core/src/revocation.rs:122", "rust/crates/aithos-core/src/revocation.rs:111", "rust/crates/aithos-core/src/revocation.rs:57", "rust/crates/aithos-core/src/gamma_replay.rs:355", "rust/crates/aithos-bundle/src/revoke.rs:105"]
    severity: elevee
    proposed_arbitration: >-
      (a) Donner a revoke_covers la chaine du revocateur et traiter la portee nue
      comme un test d'ascendance de l'emetteur, et faire echouer ferme le filtre
      de variantes de revocation.rs:111-118 ; OU (b) retirer §04.2:185-186 et
      reecrire §06.7:128-130 pour declarer la portee nue universelle. Le trou de
      vacuite du filtre doit etre ferme dans les deux options.
    status: PROPOSE-NON-IMPLEMENTE
    breaks: ["rien dans le depot : aucun vecteur ne porte d'entree de perimetre revoke, aucun scenario Gherkin n'exerce la branche nue"]
    gate_required: true
    disclosure: lifted-2026-08-04
    baseline_evidence: ["ev-cb4ff302", "ev-63e018d1", "ev-b8cee044"]
    disclosure_note: >-
      Retenu en entier du 2026-08-04T07:40Z au 2026-08-04T13:00Z sous la
      condition de blocage 9, puis publie en entier sur decision du proprietaire.
      Re-derive depuis spec/ et depuis le code a 223924e, non restitue : le
      fichier hors depot avait ete detruit. Deux points du dossier survivant sont
      contredits : la forme scopee n'est pas correctement bornee non plus (le
      filtre revocation.rs:111-118 saute quatre variantes de PerimeterEntry), et
      l'argument du durcissement retroactif unique reposait sur une phrase de
      §00.4 supprimee par c8557f4 — le lot de specification de l'orchestrateur
      lui-meme, applique 24 minutes apres le commit de la passe. Une correction
      dans un document a retire la premisse porteuse d'un constat retenu dans un
      autre ; seule la re-derivation depuis les sources l'a revele.

  - id: SPEC-CONS-13-WIRE-VERB-CREATE
    title: "'There is no wire verb create' (§04.2) face aux registres verb de K1.2"
    family: contradiction
    class: textuelle
    invariant: null
    spec_refs: ["spec/04-mandates.md:152", "spec/04-mandates.md:708", "spec/04-mandates.md:792"]
    code_refs: ["rust/crates/aithos-core/src/operation.rs", "rust/crates/aithos-core/src/mandate.rs:25-43", "rust/crates/aithos-core/src/mandate.rs:342-457"]
    proposed_arbitration: "Restreindre la phrase a la grammaire de perimetre."
    status: PROPOSE-NON-IMPLEMENTE
    breaks: []
    gate_required: false
```

---

## Note de méthode

Trois précautions ont gouverné cette passe, et elles conditionnent la lecture des
constats ci-dessus.

**Sur les affirmations d'absence.** Chacune porte sa recherche exacte, son
périmètre et sa couche, dans la phrase qui la formule. Les absences établies dans
`rust/**` (SC-08 aucun contrôle d'autorité de wrap, SC-09 aucune passe de
réparation) valent pour la **couche code** et n'établissent rien de ce que
`spec/` prévoit. Réciproquement, les
absences établies dans `spec/` (SC-03 aucun kind structurel dans le registre
fermé, SC-02 `gamma-payload` introuvable ailleurs) valent pour la **couche
spécification** et n'établissent rien de ce que le code fait — c'est précisément
pourquoi chaque constat porte son étape (2).

**Sur ce qui n'a pas pu être vérifié.** Aucune commande `cargo`, `git`, aucun test
ni build n'a été lancé, conformément à la consigne. Les conclusions sur le
comportement du code sont donc des conclusions de **lecture**, pas d'exécution :
un `#[cfg]`, une réexportation ou un appel dynamique que la lecture aurait manqué
pourrait invalider une affirmation d'absence d'appelant — **SC-07** (`check_rotation`
jamais appelée par un vérificateur) et **SC-05** (`verify_max_sessions` appelée
par aucun `src/` de crate). Les recherches ont été faites sur l'arbre source
complet, mais elles restent syntaxiques.

> **Mise à jour du 2026-08-04.** Cette phrase désignait « les constats dont le
> bord code est sous embargo » ; les deux embargos sont levés et les deux constats
> sont nommés ci-dessus. La généralisation n'avait pas d'autre motif que la
> protection de SC-05 et de SC-12.
>
> Depuis la levée, quatre gates ont été exécutés par l'orchestrateur sur ces deux
> constats seulement : `ev-cb4ff302` (espace de travail à `223924e` : 18 features /
> 114 règles / 836 scénarios / 3 577 étapes, plus les tests unitaires),
> `ev-fafd51d8` (`cb5_evidence_contracts`, 5 tests), `ev-63e018d1`
> (`@g-revocation` : 1/9/26/116) et `ev-b8cee044` (`g1_revocation`, 2 tests) —
> **tous verts**. Ce qu'ils
> changent : le critère de clôture de SC-05 et celui de SC-12 sont désormais
> attribuables depuis une ligne de base mesurée. Ce qu'ils ne changent pas :
> aucun n'exerce les branches en cause, donc **aucune affirmation d'absence de ce
> document n'est levée par eux**. Un vert ne prouve pas qu'un chemin manquant
> manque ; il prouve que rien de ce qui existe ne le réclame.

**Sur les références manquantes.** Certaines conclusions d'autres rôles ont été
retirées de l'extrait. Une trace en subsiste et a été rencontrée en cours de
route : `docs/research/topology-2026-07-28-unverified/lot-A-00-01-03-10.md`
(lignes 31 et 226) recense déjà la divergence SC-02. Elle a été trouvée
indépendamment avant cette lecture et est citée ici en corroboration, non en
source. Le dossier `features/.agents/c-headers/decisions/` et les runs
d'auditeur/correcteur du fil `c-headers` n'ont **pas** été consultés
pour établir les constats : cette passe voulait savoir ce qu'elle trouverait
seule. Aucune référence manquante n'a bloqué la lecture.
