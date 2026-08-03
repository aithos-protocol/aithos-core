# Proposition d'amendement de spécification — I3 : la clé, et le vérificateur

| Champ | Valeur |
|---|---|
| Statut | **Proposé — non appliqué.** Aucun fichier de `spec/` n'est modifié par ce document |
| Date | 2026-08-03 |
| Autorité | `features/.agents/c-headers/decisions/2026-08-03-chdr-007-012-i3-authority.md` (lecture A sur `CHDR-007` et `CHDR-012`) |
| Portée | `spec/00-overview.md`, `spec/03-headers.md`, `spec/05-delegation.md`, `spec/06-revocation.md`, `spec/09-cli-and-conformance.md`, `spec/10-threat-model.md` ; un vecteur et son générateur |
| Décideur | Propriétaire du protocole |
| Révision lue | `0148ea5`, branche `codex/audit-c-headers-r2` |
| Preuves | lecture seule de `spec/`, `rust/`, `vectors/`, `docs/audits/features/c-headers.md`. **Aucune exécution** : ni `cargo`, ni test, ni build, ni générateur lancé |

---

## 1. Préambule

### 1.1 Ce qui est proposé

Dix amendements, `SI3-1` à `SI3-10`, qui posent dans la norme les deux points
tranchés le 2026-08-03 :

1. **la ligne owner est définie par sa clé destinataire**, l'`owner_kex` publiée
   dans le document DID du sujet — pas par l'étiquette `to: "owner"` ;
2. **I3 oblige le vérificateur d'édition**, à la voix active, et non plus par une
   proposition passive dont personne n'est comptable.

### 1.2 Pourquoi maintenant

La spécification se contredit aujourd'hui à deux lignes d'intervalle.
`spec/01-identity-and-keys.md:23` **définit** :

> **owner_kex** is the recipient key of the owner's line in every header (I3).

C'est une définition, elle nomme I3, et elle désigne une clé.
`spec/03-headers.md:33-35` retire ensuite toute autorité au champ `to` :

> `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`); it is
> a routing hint only — the seal is what grants.

Et `spec/03-headers.md:36-37` énonce I3 **sans mentionner aucune clé**, ce qui
laisse `to` comme seul support apparent du contrôle — c'est-à-dire précisément le
champ que la phrase précédente déclare non autorisant. Le code a suivi la lecture
apparente : les quatre points de contrôle I3 comparent une chaîne de caractères
(`rust/crates/aithos-core/src/header.rs:71-77`, `:298-303`, `:310`), alors que
`Recipient` porte la clé (`header.rs:18`) et que `OwnerKeys::owner_kex_pub()`
(`rust/crates/aithos-core/src/keys.rs:51-53`) rend la valeur à laquelle la
comparer.

Symétriquement, la seconde moitié de I3 — « and so is the edition carrying it »
(`spec/00-overview.md:33-34`) — est écrite à la voix passive et n'oblige donc
personne. Aucun vecteur de `spec/09-cli-and-conformance.md` §9.2 ne la gate.

### 1.3 Le point dur : I3 par la clé n'a pas de témoin sans clé

Ce point n'est pas dans la décision, il en est une **condition d'applicabilité**,
et il commande la rédaction de `SI3-2`. Il doit être lu avant les amendements.

La décision veut que la garantie « Owner un-lockable-out » de
`spec/10-threat-model.md:19` devienne « une propriété qu'un tiers peut constater
sans détenir aucune clé ». Or, avec le format de ligne actuel, **c'est
cryptographiquement impossible**.

`spec/03-headers.md:121-128` fixe le sceau :

```
ss   = X25519(esk, recipient_pub)
kek  = HKDF-SHA256( ikm = ss, salt = ∅,
         info = "aithos-core/v1/hdr-kek" ‖ 0x00 ‖ epk ‖ recipient_pub )
```

La ligne publiée ne contient que `to`, `kid`, `epk`, `n`, `c`
(`spec/03-headers.md:20-22` ; `header.rs:32-38`). Un tiers connaît `epk` et
connaît `owner_kex` publique par le document DID — mais il ne détient ni `esk`
ni la moitié privée d'`owner_kex`. Il ne peut donc **pas** recalculer `ss`, donc
pas `kek`, donc pas décider si `c` s'ouvre sous `owner_kex`. La seule autre
mention de la clé destinataire est dans l'`info` du HKDF, ce qui fait échouer un
mauvais destinataire à l'ouverture — mais ne rend rien vérifiable de l'extérieur.

Conséquence, énoncée sans détour : **définir la ligne owner par sa clé rend I3
vrai, et le rend invérifiable par un tiers, tant que la ligne ne déclare pas sa
clé destinataire sur le fil.** Pour les grantees, le fil la déclare déjà :
`spec/03-headers.md:21-22` montre `to` et `kid` portant la pubkey Ed25519
multibase du destinataire, dont la conversion X25519 est normative
(`spec/01-identity-and-keys.md:52-57`). **La ligne owner est la seule ligne du
header qui ne nomme pas sa clé** : elle porte `kid: "owner-kex"`, un littéral
(`header.rs:22-28`).

`SI3-2` propose donc deux variantes, et c'est le seul endroit du document où le
propriétaire doit trancher autre chose que de la rédaction :

- **Variante A (recommandée)** — le `kid` de la ligne owner devient l'`owner_kex`
  multibase (`z6LS…`), exactement la valeur de `keys.kex` du document DID
  (`spec/01-identity-and-keys.md:94-95`). Toutes les lignes nomment alors leur
  clé, uniformément, et I3 devient une comparaison de clés vérifiable sans clé.
  `to` reste un pur indice de routage, sans rôle dans I3 : la phrase
  « routing hint only — the seal is what grants » reste vraie **et devient sans
  emploi dans le contrôle**, ce qui est exactement ce qu'elle réclame.
  Coût : changement de wire sur toute ligne owner ; `vectors/g2-rotation.json`
  porte `old_kids: ["owner-kex", …]` et est gelé — la règle 3 de
  `vectors/README.md` impose alors un nouvel id de vecteur et une redline
  explicite.
- **Variante B (repli, sans changement de wire)** — `{to: "owner", kid: "owner-kex"}`
  reste le couple réservé, mais devient *réservé* au sens fort : il MUST être
  porté par la ligne dont le destinataire est `owner_kex`, et par aucune autre.
  I3 se scinde alors en deux obligations de niveaux différents : une obligation
  structurelle pour tout vérificateur d'édition (le couple réservé est présent),
  et une obligation cryptographique pour tout vérificateur détenant `owner_kex`
  (la ligne s'ouvre). Coût : le contrôle sans clé redevient un contrôle
  d'étiquette, ce que la décision écarte — il n'est plus qu'un filet, et le
  document doit le dire.

La variante A honore la décision. La variante B la respecte en droit et l'affaiblit
en fait. Les amendements ci-dessous sont écrits pour la variante A ; chaque bloc
signale ce qui change sous B.

### 1.4 Ce que ça change pour un implémenteur tiers

La définition d'« édition valide » change. Une édition qu'une implémentation
conforme acceptait aujourd'hui peut devenir invalide demain, sans qu'un seul octet
signé ait changé de sens. C'est un durcissement rétroactif de la vérification.

### 1.5 Version de spécification

Deux plans de version existent aujourd'hui, tous deux décrits en
`spec/00-overview.md:61-99` : le plan de publication manifeste/Gamma
(`aithos-core`, profils `"1.0.0-draft.1"` et `"1.0.0-draft.2"`) et le plan
mandat (`aithos-mandate-core`, `draft.1`, `draft.2`, `draft.3` réservé). Le
bandeau du document annonce par ailleurs `spec/00-overview.md:3` :

> **Status: DRAFT.** Aithos Core, wire version `aithos-core: "1.0.0-draft.1"`.

**Proposition : ne pas ouvrir de profil manifeste `"1.0.0-draft.3"`.** Trois
motifs.

1. Les profils de `spec/00-overview.md:66-77` gouvernent des **introductions de
   constructions signées** — « introduces only historical Gamma v1 entries »,
   « introduces only Gamma v2 entries and the K1-B operation, changeset, and
   evidence references ». L'amendement I3 n'introduit aucune construction signée
   et ne change aucun octet signé.
2. Un durcissement gated sur le profil le plus récent est **contournable en
   publiant sous l'ancien**. C'est exactement le producteur que `CHDR-007`
   nomme : la branche déléguée de `Bundle::verify`
   (`rust/crates/aithos-bundle/src/bundle.rs:1664`, `m.version == CORE_DRAFT2_VERSION`).
   Une règle de sécurité qui ne lie que `draft.3` ne lie rien.
3. L'obligation doit donc lier **tous** les profils `aithos-core`, historiques
   compris — ce que le modèle monotone de §0.4 ne sait pas exprimer.

L'incrément proposé est donc porté par le **bandeau de la série DRAFT**, et le
fait rétroactif est écrit noir sur blanc dans §0.4 (`SI3-10`). Le propriétaire
qui préférerait tout de même un profil manifeste devra alors répondre à la
question du point 2 : que fait un vérificateur devant une édition `draft.2`
publiée après l'amendement.

Le bandeau `spec/00-overview.md:3` est de surcroît **déjà périmé** : il annonce
`"1.0.0-draft.1"` alors que `spec/00-overview.md:70` et
`spec/02-content-tree.md:226` décrivent `"1.0.0-draft.2"` comme profil
d'émission courant. `SI3-10` répare les deux d'un geste.

Ce document ne statue pas sur la version du crate Rust : la décision l'a déjà
fait (« bump majeur de version du crate », lot B).

---

## 2. Amendements

Chaque amendement cite le fichier, la section et les numéros de ligne **à la
révision `0148ea5`**.

---

### `SI3-1` — `spec/00-overview.md` §0.2, invariant 3 (l. 33-34)

**Objet.** Énoncer I3 à la voix active, sur la clé, avec un obligé nommé.

**Texte actuel** (`spec/00-overview.md:33-34`) :

```
3. **I3 — Owner line.** Every header MUST contain a line for the owner. A header
   without one is invalid, and so is the edition carrying it.
```

**Texte proposé** :

```
3. **I3 — Owner line.** Every `key_versions[*].lines` of every header MUST contain
   the owner line: the line whose recipient key is the subject's `owner_kex`, as
   published in the DID document (§01.1, §01.4, §03.1). A header without one is
   invalid. An edition verifier MUST parse every header the edition pins and MUST
   reject the edition if any key version of any of them has no owner line. The
   routing label `to` never establishes the owner line and never satisfies I3.
```

**Justification.** La voix passive est la cause directe de l'inapplicabilité
constatée : `Bundle::verify` (`bundle.rs:1654-1769`) contrôle le document DID, la
chaîne de manifestes, les signatures, `prev_hash`, les forks, les digests SHA-256,
les liens gamma et les racines Merkle — et n'appelle jamais `Header::validate`.
Le seul contact de la vérification avec les headers est
`Bundle::header_hash_at` (`rust/crates/aithos-bundle/src/state.rs:58-68`), qui
désérialise le fichier en `serde_json::Value` **opaque** pour en calculer
`BLAKE3(JCS(…))` : un header sans ligne owner y produit un digest parfaitement
valide, plié dans la racine d'état, épinglé, puis signé. La mention explicite de
`owner_kex` aligne §0.2 sur `spec/01-identity-and-keys.md:23`.

*Variante B :* remplacer « the line whose recipient key is the subject's
`owner_kex` » par « the line carrying the reserved owner routing pair
(§03.1), which MUST be sealed to the subject's `owner_kex` ».

---

### `SI3-2` — `spec/03-headers.md` §3.1, définition de la ligne (l. 20, 33-37)

**Objet.** Définir la ligne owner par sa clé destinataire, lui donner un témoin
sur le fil, et rendre `to` définitivement sans emploi dans le contrôle.

**Texte actuel** (`spec/03-headers.md:33-37`) :

```
- `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`); it is
  a routing hint only — the seal is what grants. Recipients try lines addressed to
  their `kid`.
- **I3:** every `key_versions[*].lines` MUST include the owner line. An edition whose
  any header violates this is invalid.
```

**Texte proposé** (variante A) :

```
- `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`); it is
  a routing hint only — the seal is what grants. Recipients try lines addressed to
  their `kid`. No verifier decides anything from `to`.
- `kid` names the line's recipient **key**: the grantee's multibase Ed25519 pubkey,
  whose X25519 counterpart is obtained by the normative map of §01.2, or — for the
  owner line — the subject's `owner_kex` in multibase (`z6LS…`), byte-identical to
  `keys.kex` of the subject's DID document (§01.4). Two lines of one key version
  MUST NOT carry the same `kid`.
- The **owner line** of a key version is the line whose recipient key is the
  subject's `owner_kex`. The seal identifies it, not the label: a line labelled
  `"owner"` and sealed to any other key is **not** the owner line, and a line
  labelled otherwise but sealed to `owner_kex` **is**.
- **I3:** every `key_versions[*].lines` MUST include the owner line. A header
  violating this is invalid. An edition verifier MUST reject an edition that pins
  such a header (§0.2, §9.4). Every verifier MUST check, without any key, that some
  line of every key version declares `owner_kex` as its `kid`; a verifier holding
  `owner_kex` MUST additionally check that that line opens under it, and MUST reject
  the header when it does not.
```

L'exemple JSONC de `spec/03-headers.md:20` devient alors :

```jsonc
{ "to": "owner",            "kid": "z6LSOwnerKex…",  "n": "…", "c": "…" },
```

**Justification.** Trois pièces. `spec/01-identity-and-keys.md:23` définit
`owner_kex` comme « the recipient key of the owner's line in every header (I3) » :
la définition par la clé est déjà dans la norme, et §3.1 est le seul endroit qui
la contredit. `spec/03-headers.md:33-35` déclare `to` non autorisant : lier I3 à
`to` obligeait à tenir §3.1 pour contradictoire avec elle-même. Enfin, la
déclaration du `kid` est ce qui donne à la définition un témoin vérifiable sans
clé (§1.3 ci-dessus) ; sans elle, l'obligation de `SI3-1` porte sur une propriété
qu'aucun tiers ne peut constater.

La phrase « `to` is a routing hint only — the seal is what grants » est conservée
mot pour mot et **renforcée** : après cet amendement, plus aucune règle normative
ne lit `to`.

*Variante B :* supprimer le second bullet (`kid` names the key), et remplacer la
dernière phrase du bullet I3 par : « Every verifier MUST check, without any key,
that some line of every key version carries the reserved owner pair
`{"to": "owner", "kid": "owner-kex"}`, which MUST NOT be carried by a line sealed
to any key other than `owner_kex`; a verifier holding `owner_kex` MUST
additionally check that that line opens under it. A keyless verifier cannot
establish the seal, and this residual gap is deliberate. »

---

### `SI3-3` — `spec/03-headers.md` §3.2 Reading (l. 41-44)

**Objet.** Empêcher que la lecture, qui procède par étiquette, soit relue comme
une preuve d'appartenance.

**Texte actuel** (`spec/03-headers.md:41-44`) :

```
To open node N: pick the `key_version` matching the target blob's index entry, find a
line whose `kid` is mine (or `owner`), unseal → DK → derive down (§02.5) → decrypt.
The owner always resolves via `owner-kex`; a grantee via its keypair. No network, no
per-read state.
```

**Texte proposé** (variante A) :

```
To open node N: pick the `key_version` matching the target blob's index entry, find a
line whose `kid` is mine, unseal → DK → derive down (§02.5) → decrypt. The owner
resolves the line whose `kid` is its `owner_kex`; a grantee its own. `kid` orders the
attempts and nothing else: a reader that finds no matching line MAY try the remaining
lines, and a successful unseal — never a label — is what proves the line was its own.
No network, no per-read state.
```

**Justification.** §3.2 est aujourd'hui le seul endroit de la norme qui autorise
un lecteur à s'identifier par `"owner"`. Une fois I3 défini par la clé, cette
phrase devient la dernière source d'ambiguïté sur ce que `to`/`kid` établissent.
Le code s'appuie déjà sur l'essai : `Header::open` recalcule l'AAD et tente
l'ouverture (`header.rs:228`).

*Variante B :* conserver « (or `owner`) » et ajouter seulement la dernière phrase
(« `kid` orders the attempts and nothing else … »).

---

### `SI3-4` — `spec/03-headers.md` §3.4, vérification mécanique (l. 93-96)

**Objet.** Étendre la vérification mécanique de la rotation au maintien de la
ligne owner ainsi définie.

**Texte actuel** (`spec/03-headers.md:93-96`) :

```
re-establishes that path in one entry and touches no other line. Verification is
mechanical: the new version's lines MUST equal the previous lines minus the revoked
(plus, in the exactly-N case, recipients ⊆ P's header), and an up-link wrap whose
author does not hold P is rejected.
```

**Texte proposé** :

```
re-establishes that path in one entry and touches no other line. Verification is
mechanical: the new version's lines MUST equal the previous lines minus the revoked
(plus, in the exactly-N case, recipients ⊆ P's header), the new version MUST carry the
owner line as defined in §3.1 — the revoker re-seals DK' to the subject's `owner_kex`
read from the DID document, never to whatever key the previous owner line used — and an
up-link wrap whose author does not hold P is rejected.
```

**Justification.** `Header::check_rotation` (`header.rs:274-303`) contrôle
aujourd'hui la ligne owner par `new.lines.iter().any(|l| l.to == OWNER_LABEL)`
(`header.rs:298`). `spec/05-delegation.md:85-88` autorise explicitement un
révocateur « owner **or** ancestor » à re-sceller les lignes des survivants,
ligne owner comprise ; un rotateur émettant `{ to: "owner", kid: <son propre kid,
déjà présent en v1> }` traverse donc les deux gardes à la fois — la garde
anti-clandestin ne voit rien puisque le `kid` existait, la garde I3 ne voit rien
puisque l'étiquette dit `"owner"`. La formule « read from the DID document »
nomme la source de vérité, qui est celle que le code emploie déjà côté écrivain
(`Bundle::owner_kex_recipient`, `rust/crates/aithos-bundle/src/grants.rs:171-174`).

**Rédaction volontairement additive.** La proposition « the new version's lines
MUST equal the previous lines minus the revoked » est reprise **verbatim**, sans
un caractère de changement. Elle est l'ancrage de `CHDR-024`, que cette
proposition ne traite pas (§6) : la laisser intacte évite qu'un futur amendement
d'égalité entre en collision avec celui-ci.

---

### `SI3-5` — `spec/05-delegation.md` §5.5 (l. 85-88)

**Objet.** Dire de quelle clé le révocateur re-scelle la ligne owner.

**Texte actuel** (`spec/05-delegation.md:85-88`) :

```
- A delegate revokes **its own** children without touching siblings, cousins, or the
  owner's other grants on the same node: it rotates the node key and republishes the
  header **omitting the revoked child's line but keeping every other line** — including
  lines it did not create (those it re-seals under the new DK using its own access).
```

**Texte proposé** :

```
- A delegate revokes **its own** children without touching siblings, cousins, or the
  owner's other grants on the same node: it rotates the node key and republishes the
  header **omitting the revoked child's line but keeping every other line** — including
  lines it did not create (those it re-seals under the new DK using its own access).
  The owner line is re-sealed to the subject's `owner_kex` read from the DID document
  (§03.1), never to the recipient key the previous owner line happened to carry: a
  rotation that reproduces a wrong owner line propagates it, and I3 makes the whole
  edition invalid.
```

**Justification.** §5.5 est le passage qui met un tiers — un délégué — en position
d'écrire la ligne owner. La décision le relève comme le motif pour lequel la
défense « ce n'est que de l'auto-sabotage » ne tient pas. Le contrôle d'autorité
qui bornerait ce pouvoir n'existe pas dans le code
(`docs/proposals/header-rotation-authority.md`, statut *Proposé — non adopté*,
qui constate en §« État de l'implémentation » que `check_rotation` ne vérifie
« aucun contrôle d'autorité »). Tant que cette proposition-là n'est pas adoptée,
la seule barrière contre une ligne owner falsifiée par un délégué est I3 défini
par la clé.

---

### `SI3-6` — `spec/06-revocation.md` §6.2, procédure (l. 33)

**Objet.** Le pseudo-code normatif dit « + owner » ; sous la nouvelle définition,
c'est une clé, pas un mot.

**Texte actuel** (`spec/06-revocation.md:33`) :

```
         header[N].new = { lines: reseal DK' to all survivors + owner }   # not M
```

**Texte proposé** :

```
         header[N].new = { lines: reseal DK' to all survivors
                                  + the owner line, sealed to owner_kex (§03.1) }  # not M
```

**Justification.** `spec/06-revocation.md:33` est la seule occurrence de « owner »
dans le corps procédural de §06 (vérifié par recherche sur le fichier). Laissée
telle quelle, elle continue de suggérer un rôle plutôt qu'une clé, sur le chemin
même que `SI3-4` et `SI3-5` durcissent. Trois sites de production lisent
aujourd'hui l'étiquette pour décider quelle ligne remplacer — `revoke.rs:180`,
`structure.rs:259`, `vault.rs:375` remplacent toute ligne dont `line.to == "owner"`
par `owner_kex_recipient()` : leur comportement est correct, leur critère ne l'est
pas, et §6.2 est le texte dont ils dérivent.

---

### `SI3-7` — `spec/10-threat-model.md` §10.1, ligne « Owner un-lockable-out » (l. 19)

**Objet.** Nommer la contre-mesure exécutable, pas seulement l'invariant.

**Texte actuel** (`spec/10-threat-model.md:19`) :

```
| Owner un-lockable-out | owner line mandatory in every header (I3); owner holds root of all authority |
```

**Texte proposé** :

```
| Owner un-lockable-out | owner line mandatory in every header, identified by its recipient key and enforced by the edition verifier (I3, §03.1, §09.4); owner holds root of all authority |
```

**Justification.** `spec/10-threat-model.md:19` ne cite qu'une contre-mesure pour
cette menace, et cette contre-mesure n'était imposée par aucun vérificateur
d'édition : `Bundle::verify` (`bundle.rs:1654-1769`) et
`publication::cold_verify` (`rust/crates/aithos-bundle/src/publication.rs:836-939`)
sont l'un et l'autre muets sur I3. Une table de menaces qui cite une contre-mesure
inappliquée est une table fausse.

---

### `SI3-8` — `spec/09-cli-and-conformance.md` §9.2 (l. 46-47)

**Objet.** Faire entrer les cas I3 dans la liste des vecteurs normatifs.

**Texte actuel** (`spec/09-cli-and-conformance.md:46-47`) :

```
Both success and every fail-closed case (unauthorized revocation, over-wide
sub-mandate, N+1 action, expired heartbeat) get a vector. Session-2 additions MUST
```

**Texte proposé** :

```
Both success and every fail-closed case (unauthorized revocation, over-wide
sub-mandate, N+1 action, expired heartbeat) get a vector. I3 gets its own family
(§03.1): a header whose every key version carries the owner line → valid; a header
one of whose key versions carries no owner line at all → the edition is rejected; a
header whose line labelled `"owner"` is sealed to a key that is not the subject's
`owner_kex` → rejected; a header whose owner line is not labelled `"owner"` but is
sealed to `owner_kex` → valid, proving the label decides nothing in either
direction. Each case states which verifier tier it binds: keyless (edition
verification) or `owner_kex`-bearing. Session-2 additions MUST
```

**Justification.** La décision constate que « aucun vecteur de conformité §9.2 ne
gate ce cas », et en fait le motif pour lequel le lot de spécification précède le
lot de correction. §9.2 est aussi la seule liste dont `docs/CONFORMANCE.md` §2
dérive sa table de couverture.

---

### `SI3-9` — `spec/09-cli-and-conformance.md` §9.4, niveau *Core reader* (l. 92-93)

**Objet.** Le niveau de conformité qui revendique la vérification d'édition doit
porter l'obligation.

**Texte actuel** (`spec/09-cli-and-conformance.md:92-93`) :

```
- **Core reader**: resolves DID, opens headers it has lines for, derives, decrypts,
  verifies editions + gamma. MUST implement the fork rule (§02.6) fail-closed.
```

**Texte proposé** :

```
- **Core reader**: resolves DID, opens headers it has lines for, derives, decrypts,
  verifies editions + gamma. MUST implement the fork rule (§02.6) fail-closed, and
  MUST reject an edition pinning a header that violates I3 (§03.1) — without holding
  any key, and on every `aithos-core` manifest profile.
```

**Justification.** §9.4 est l'endroit où une implémentation déclare ce qu'elle
tient, et « An implementation states which levels it claims; the vectors gate
each » (`spec/09-cli-and-conformance.md:100`). Sans cette ligne, `SI3-1` n'a pas
de niveau porteur et un implémenteur peut revendiquer *Core reader* sans faire le
contrôle. Le rattachement au niveau *reader* (et non *issuer*) est délibéré : la
vérification d'édition est une capacité de lecteur.

---

### `SI3-10` — `spec/00-overview.md` bandeau (l. 3) et §0.4 (l. 66-77)

**Objet.** Incrémenter la version de spécification, dire que le durcissement est
rétroactif, et réparer un bandeau périmé.

**Texte actuel** (`spec/00-overview.md:3`) :

```
> **Status: DRAFT.** Aithos Core, wire version `aithos-core: "1.0.0-draft.1"`.
```

**Texte proposé** :

```
> **Status: DRAFT.** Aithos Core, specification revision `2026-08-03-i3-authority`.
> Manifest publication profiles: `aithos-core: "1.0.0-draft.1"` (historical
> verification) and `"1.0.0-draft.2"` (current issuance) — §0.4.
```

**Texte actuel** (`spec/00-overview.md:74-77`) :

```
Version order is causal, never inferred from physical JSONL order: draft1/v1 may
lead to draft1/v1 or draft2/v2, while draft2/v2 never leads back. Missing, mixed on
one introducing edge, or unknown profiles fail closed. Historical manifests and
entries are never rewritten or assigned synthetic references.
```

**Texte proposé** (paragraphe ajouté à la suite, l. 77) :

```
Version order is causal, never inferred from physical JSONL order: draft1/v1 may
lead to draft1/v1 or draft2/v2, while draft2/v2 never leads back. Missing, mixed on
one introducing edge, or unknown profiles fail closed. Historical manifests and
entries are never rewritten or assigned synthetic references.

A profile gates the introduction of signed constructs; it never gates a verification
rule. The I3 obligation of §0.2 introduces no signed construct and changes no signed
byte: it binds every `aithos-core` profile, historical ones included. A rule that
bound only the newest profile would be escaped by publishing under an older one, and
would bind nothing. Editions published before specification revision
`2026-08-03-i3-authority` are therefore re-verified under it; this is the one
retroactive tightening of this series, and it is stated here rather than hidden in a
profile.
```

**Justification.** Le durcissement change la définition de « valide » pour un
tiers, ce que la décision demande d'assumer par un incrément. Le bandeau est
aujourd'hui incohérent avec le corps du document : il annonce `"1.0.0-draft.1"`
tandis que `spec/00-overview.md:70` et `spec/02-content-tree.md:226` décrivent
`"1.0.0-draft.2"` comme profil courant. Le refus d'un profil `draft.3` est motivé
en §1.5.

---

## 3. Le vecteur de conformité

### 3.1 Identité et emplacement

| Champ | Valeur |
|---|---|
| Fichier | `vectors/c3-owner-line.json` |
| `vector` | `"C3"` |
| Famille | C, la même que `c1-header-seal.json` (« C1+C2 ») ; C3 est le premier id libre |
| Propriété | `core` — entrée à ajouter dans `vectors/ownership.json` (`kind: "vector"`, `owner: "core"`, `sha256`), sans quoi `rust/crates/aithos-bundle/tests/vectors_ownership.rs` passe au rouge |
| Consommateur Rust attendu | `rust/crates/aithos-core/tests/c3_owner_line.rs` (contrôles de header) et un cas d'édition côté `aithos-bundle` |

`c1-header-seal.json` n'est **pas** modifié : la règle 3 de `vectors/README.md`
(« Frozen once green ») l'interdit, et son sha256 est épinglé dans
`vectors/ownership.json:35-40`.

### 3.2 Entrées communes

Toutes reprises des vecteurs déjà gelés, pour que C3 se rattache à l'existant
plutôt que d'inventer un cast :

| Champ | Valeur | Provenance |
|---|---|---|
| `seed_hex` | `000102…1e1f` | `a1-genesis.json`, `a2-did.json`, `gen-g.py:25` |
| `subject_did` | `did:aithos:z6Mkopv…tvZHr` | `a2-did.json` (`did`), identique à `c1-header-seal.json` |
| `owner_kex_pub_hex` | `2a87b432…834165` | `a1-genesis.json` (`owner_kex_pub_hex`) — **la même valeur** que `c1-header-seal.json` (`owner_pub_hex`) |
| `owner_kex_pub_multibase` | `z6LSeYCJg2G3i6zEiYd2bvnacfR8EnQoUUv3315nBbJL85sS` | `a1-genesis.json` ; c'est `keys.kex` du document DID de `a2-did.json` |
| `node` | `/e/circle` | `c1-header-seal.json` |
| `key_version` | `1` | `c1-header-seal.json` |
| `dk_hex` | `c8c9ca…e6e7` | `c1-header-seal.json` |
| `stranger_pub_hex` | `7d34a481…268a44` | `c1-header-seal.json` (`grantee_pub_hex`) — une clé réelle, non owner |

### 3.3 Forme JSON

```jsonc
{
  "vector": "C3",
  "description": "I3 owner line (spec 03.1): the owner line is identified by its recipient key — the subject's owner_kex published in the DID document — never by the `to` label. Four headers, one positive per direction and two negatives, each stating the verifier tier it binds. Generated independently (Python blake3 + PyNaCl + manual RFC5869 HKDF + base58).",

  "seed_hex": "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
  "subject_did": "did:aithos:z6MkopvL9x5EQew3DyVAqyGNfQpsY116sA7CjRstz8NtvZHr",
  "node": "/e/circle",
  "key_version": 1,
  "dk_hex": "c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7",

  "owner_kex_sk_hex": "7d0871c1…de68373",          // = c1-header-seal.json
  "owner_kex_pub_hex": "2a87b432…f5834165",
  "owner_kex_pub_multibase": "z6LSeYCJg2G3i6zEiYd2bvnacfR8EnQoUUv3315nBbJL85sS",
  "stranger_sk_hex": "2121…2121",                   // = c1 grantee
  "stranger_pub_hex": "7d34a481…e8268a44",
  "stranger_multibase": "z6LS…",                    // multibase x25519 of stranger_pub

  "cases": [
    {
      "name": "owner_line_present",
      "verdict": "valid",
      "tier": "keyless",
      "proves": "a key version carrying a line whose kid is owner_kex satisfies I3; the edition verifies",
      "header": { "object": "header", "v": 1, "node": "/e/circle",
                  "key_versions": { "1": { "lines": [ /* owner line, stranger line */ ] } } }
    },
    {
      "name": "no_owner_line_at_all",
      "verdict": "invalid",
      "must_fail": "MissingOwnerLine",
      "tier": "keyless",
      "proves": "a key version with no owner line makes the header invalid AND the edition pinning it invalid — the half of I3 no verifier enforced",
      "header": { /* key_versions.1.lines = [ stranger line only ] */ }
    },
    {
      "name": "owner_label_foreign_key",
      "verdict": "invalid",
      "must_fail": "MissingOwnerLine",
      "tier": "keyless",
      "proves": "a line labelled to=\"owner\" whose declared recipient key is not owner_kex is not the owner line; the label grants nothing",
      "header": { /* one line: to=\"owner\", kid=stranger_multibase, sealed to stranger_pub */ }
    },
    {
      "name": "owner_label_foreign_seal",
      "verdict": "invalid",
      "must_fail": "MissingOwnerLine",
      "tier": "owner_kex",
      "proves": "a line that DECLARES owner_kex as its kid but is sealed to another key is rejected by a verifier holding owner_kex; a keyless verifier accepts it, and that residual gap is the documented boundary of §3.1",
      "header": { /* one line: to=\"owner\", kid=owner_kex_multibase, sealed to stranger_pub */ }
    },
    {
      "name": "unlabelled_owner_line",
      "verdict": "valid",
      "tier": "keyless",
      "proves": "a line sealed to owner_kex satisfies I3 even when `to` names something else — the label decides nothing in either direction",
      "header": { /* one line: to=stranger_multibase, kid=owner_kex_multibase, sealed to owner_kex */ }
    }
  ]
}
```

Chaque `lines[*]` porte les cinq champs de `spec/03-headers.md:20-22` —
`to`, `kid`, `epk`, `n`, `c` — en hex sur disque, tous les éphémères et nonces
étant des **entrées fixes** (`spec/03-headers.md:137-139`).

### 3.4 Ce que chaque cas doit prouver, et pourquoi ce jeu-là

Les deux négatifs exigés par la commande sont `no_owner_line_at_all` et
`owner_label_foreign_key`. Trois remarques.

1. `no_owner_line_at_all` est le seul cas qui gate la **moitié édition** de I3.
   Il doit être consommé deux fois : par un test de header (`Header::validate`
   échoue) *et* par un test d'édition (`Bundle::verify` échoue sur un bundle dont
   un `header.json` pinné porte ce contenu). Sans le second, le vecteur regate ce
   qui l'était déjà (`header.rs:308-315`) et laisse le finding ouvert.
2. `owner_label_foreign_key` et `owner_label_foreign_seal` sont **deux** négatifs
   distincts, et c'est le cœur du §1.3 : le premier est détectable sans clé (le
   `kid` déclaré n'est pas `owner_kex`), le second ne l'est pas (le `kid` déclaré
   est bon, le sceau ment). Les séparer inscrit dans le corpus la frontière exacte
   de ce qu'un tiers peut constater. Sous la **variante B**, `owner_label_foreign_key`
   change de niveau : il devient lui aussi `tier: "owner_kex"`, puisque plus rien
   sur le fil ne nomme la clé. Le vecteur rend donc le coût de B mesurable.
3. `unlabelled_owner_line` est le positif symétrique, et il est indispensable :
   sans lui, une implémentation qui aurait simplement remplacé un littéral par un
   autre littéral passerait les trois autres cas.

Le positif `owner_line_present` doit réutiliser **exactement** les entrées de la
ligne owner de `c1-header-seal.json` (`esk_hex` `78797a…9697`, `n_hex`
`000102…1617`, même `dk_hex`, même `node`, même `key_version`), de sorte que sa
ligne soit **byte-identique** à `c1-header-seal.json.owner_line`. C3 se rattache
ainsi à C1 par construction, et non par recopie.

---

## 4. Le générateur

### 4.1 Conventions relevées dans `vectors/`

Lecture de `gen-g.py`, `gen-h.py` et `gen-cb2-bundle-boundaries.py` (28
générateurs `gen-*.py` au total dans `vectors/`) :

| Convention | Constat |
|---|---|
| Shebang + docstring | `#!/usr/bin/env python3` puis une docstring qui liste les fichiers produits, ce que chacun prouve, et la règle de seconde implémentation (`gen-g.py:1-15`) |
| Dépendances | `blake3`, `PyNaCl` (`nacl.bindings`, `nacl.signing`), `base58`, `hashlib`, `json` — jamais la référence Rust (`gen-g.py:13-14, 17-23`) |
| Helpers canoniques | `jcs()` = `json.dumps(sort_keys=True, separators=(",",":"), ensure_ascii=False)` ; `derive(ctx,key)` = `blake3(key, derive_key_context=ctx)` ; `multibase_ed(pub)` = `"z" + base58(b"\xed\x01" + pub)` (`gen-g.py:29-38`) |
| Auto-validation | reproduire une valeur d'un vecteur **déjà gelé** avant d'écrire quoi que ce soit, et `assert` : `gen-g.py:150-153` recharge `b2-derivation.json` et refait `folder1_key_hex` ; `gen-h.py:7-8` fait de même |
| Aléa | éphémères et nonces sont des **constantes injectées**, en motifs lisibles (`bytes.fromhex("55"*32)`, `"66"*32`, `"77"*24` — `gen-g.py:110-112`) ou en rampes d'octets (`c1-header-seal.json` : `78797a7b…`, `000102…`) |
| Sortie | `json.dump(obj, f, indent=2, ensure_ascii=False)` + `"\n"` final, puis `print(f"wrote …")` (`gen-g.py:243-249`) |
| Mode `--check` | les générateurs récents ajoutent `argparse` avec `--check` / `--output`, comparent les octets et sortent en erreur si le fichier n'est pas reproductible (`gen-cb2-bundle-boundaries.py`, fonctions `encoded()` et `main()`) |
| Entrée `ownership.json` | `kind: "tooling"`, `owner: "core"`, **sans** `sha256` pour les générateurs de la famille lettre (cf. entrées `gen-g.py`, `gen-h.py`) |

### 4.2 Un générateur ou deux ?

**Proposition : un seul, `vectors/gen-c.py`, qui produit `c3-owner-line.json` et
qui *vérifie* `c1-header-seal.json` sans jamais le réécrire.**

Motifs.

1. Le nommage par lettre est la convention de la famille : `gen-f.py` produit
   `f1`/`f2`/`f3`, `gen-g.py` produit `g1`/`g2`/`g3`, `gen-h.py` produit `h1`. Un
   `gen-c1-header-seal.py` serait le seul générateur de vecteur core nommé par
   fichier plutôt que par famille.
2. C3 et C1 partagent la clé owner, le DID, le nœud, la version, la DK et
   l'éphémère de la ligne owner (§3.4). Faire dériver les deux d'un même corps
   rend ce partage **structurel** ; deux générateurs le rendraient déclaratif,
   c'est-à-dire copiable et dérivable.
3. Cela solde `CHDR-025` de la seule façon compatible avec la règle 3 de
   `vectors/README.md`. Le constat est établi : `c1_header_seal.rs:2-3` déclare
   « Expected ciphertexts generated independently (Python PyNaCl + manual
   RFC 5869 HKDF) » et `c1-header-seal.json` le répète dans `description`, mais
   **aucun `gen-c1*` n'a jamais existé dans le dépôt** — `vectors/` contient 28
   générateurs, dont aucun pour la famille C (vérifié par listage). C'est
   l'obligation `TARGETED` ouverte par la revue d'impact `b-derivation` du
   2026-08-03. `gen-c.py` ne « regénère » donc pas C1 : il le **reconstruit et
   l'asserte**, ce qui transforme une revendication de provenance en propriété
   mécanique, sans toucher un octet gelé.
   *Note connexe, non amendée ici :* `docs/CONFORMANCE.md:50-52` affirme « All
   vectors are generated by an independent Python implementation
   (`vectors/gen-*.py` …) ». Cette phrase est fausse tant que `gen-c.py` n'existe
   pas ; elle redevient vraie ensuite.

Une seule raison plaiderait pour deux générateurs : si le propriétaire jugeait
qu'un générateur qui échoue sur C1 doit pouvoir produire C3 quand même. C'est
précisément ce qu'il ne faut pas : un C3 émis pendant que C1 dérive serait un
vecteur dont la base de comparaison n'est plus établie.

### 4.3 Squelette proposé

À écrire dans `vectors/gen-c.py`. Le squelette est complet sur les conventions
et les primitives ; ce qui reste au propriétaire est le choix des éphémères de C3
et le remplissage des cas.

```python
#!/usr/bin/env python3
"""Independent generator for the C conformance vectors (headers, spec 03).

  c1-header-seal.json  header line seal/open (C1) and wrap (C2), spec 03.8.
                       NEVER rewritten — this generator RECONSTRUCTS it and
                       asserts it byte for byte. The file is frozen (README
                       rule 3) and its sha256 is pinned in ownership.json.
                       This closes the standing claim of independent
                       generation that had no generator in the repository.
  c3-owner-line.json   I3 owner line (spec 03.1): the owner line is the line
                       whose recipient key is the subject's owner_kex, never
                       the line whose `to` says "owner". One positive per
                       direction, two negatives, each tagged with the verifier
                       tier it binds (keyless / owner_kex-bearing).

Second-implementation rule: blake3 + PyNaCl + hmac/hashlib (manual RFC 5869
HKDF) + base58, never the Rust reference. Auto-validated against the frozen
a1-genesis.json before anything is written.

Usage: python3 gen-c.py [--check]   (from vectors/)
"""

import argparse
import hmac
import json
from hashlib import sha256
from pathlib import Path

import base58
import blake3
from nacl.bindings import (
    crypto_aead_xchacha20poly1305_ietf_encrypt,
    crypto_scalarmult,
    crypto_scalarmult_base,
)

HERE = Path(__file__).resolve().parent

SEED = bytes.fromhex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f")

# --- primitives (spec 00.3, 03.8) -------------------------------------------

def derive(context: str, key: bytes) -> bytes:
    """BLAKE3 derive_key — spec 00.3."""
    return blake3.blake3(key, derive_key_context=context).digest()


def multibase_x(pub: bytes) -> str:
    """x25519-pub multicodec 0xec01, base58btc — spec 00 encodings."""
    return "z" + base58.b58encode(b"\xec\x01" + pub).decode()


def hkdf_sha256(ikm: bytes, salt: bytes, info: bytes, length: int = 32) -> bytes:
    """RFC 5869, written out — the second implementation must not import the
    same library the reference uses."""
    prk = hmac.new(salt or b"\x00" * 32, ikm, sha256).digest()
    out, t, counter = b"", b"", 1
    while len(out) < length:
        t = hmac.new(prk, t + info + bytes([counter]), sha256).digest()
        out += t
        counter += 1
    return out[:length]


def line_aad(subject_did: str, node: str, key_version: int) -> bytes:
    """spec 03.8: purpose NUL did NUL node NUL key_version (decimal ASCII)."""
    return (
        b"aithos-core/v1/header-line" + b"\x00"
        + subject_did.encode() + b"\x00"
        + node.encode() + b"\x00"
        + str(key_version).encode()
    )


def wrap_aad(subject_did: str, wrapped_node: str, key_version: int) -> bytes:
    return (
        b"aithos-core/v1/tagwrap" + b"\x00"
        + subject_did.encode() + b"\x00"
        + wrapped_node.encode() + b"\x00"
        + str(key_version).encode()
    )


def seal_line(esk: bytes, recipient_pub: bytes, dk: bytes, nonce: bytes, aad: bytes):
    """spec 03.8 line: ECIES X25519 + HKDF-SHA256 + XChaCha20-Poly1305.
    Returns (epk, ciphertext)."""
    epk = crypto_scalarmult_base(esk)
    ss = crypto_scalarmult(esk, recipient_pub)
    kek = hkdf_sha256(
        ikm=ss,
        salt=b"",
        info=b"aithos-core/v1/hdr-kek" + b"\x00" + epk + recipient_pub,
    )
    return epk, crypto_aead_xchacha20poly1305_ietf_encrypt(dk, aad, nonce, kek)


def seal_wrap(via_key: bytes, dk: bytes, nonce: bytes, aad: bytes) -> bytes:
    """spec 03.8 wrap: key = derive("aithos-core/v1/wrap", K_via)."""
    return crypto_aead_xchacha20poly1305_ietf_encrypt(
        dk, aad, nonce, derive("aithos-core/v1/wrap", via_key)
    )


def line(to: str, kid: str, epk: bytes, nonce: bytes, c: bytes) -> dict:
    """spec 03.1: the five wire fields, hex on disk."""
    return {"to": to, "kid": kid, "epk": epk.hex(), "n": nonce.hex(), "c": c.hex()}


# --- cast, auto-validated against the frozen A1 vector -----------------------

OWNER_SK = derive("aithos-core/v1/owner-kex", SEED)
OWNER_PUB = crypto_scalarmult_base(OWNER_SK)

STRANGER_SK = bytes.fromhex("21" * 32)          # = c1 grantee_sk_hex
STRANGER_PUB = crypto_scalarmult_base(STRANGER_SK)

NODE = "/e/circle"
KEY_VERSION = 1
DK = bytes.fromhex("c8c9cacbcccdcecfd0d1d2d3d4d5d6d7d8d9dadbdcdddedfe0e1e2e3e4e5e6e7")

# Ephemerals and nonces are INPUTS (spec 03.8 / 09.2). C1's are frozen; C3's
# continue the same byte-ramp convention. TODO(owner): confirm the C3 ramps.
C1_OWNER_ESK = bytes.fromhex("78797a7b7c7d7e7f808182838485868788898a8b8c8d8e8f9091929394959697")
C1_OWNER_N = bytes.fromhex("000102030405060708090a0b0c0d0e0f1011121314151617")
C1_GRANTEE_ESK = bytes.fromhex("98999a9b9c9d9e9fa0a1a2a3a4a5a6a7a8a9aaabacadaeafb0b1b2b3b4b5b6b7")
C1_GRANTEE_N = bytes.fromhex("18191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f")
C3_ESK = {  # one per non-C1 line, deterministic and readable
    "stranger": bytes.fromhex("b8" * 32),
    "foreign_a": bytes.fromhex("b9" * 32),
    "foreign_b": bytes.fromhex("ba" * 32),
    "unlabelled": bytes.fromhex("bb" * 32),
}
C3_NONCE = {k: bytes([0xc0 + i]) * 24 for i, k in enumerate(C3_ESK)}


def crosscheck_a1() -> str:
    """gen-g.py:150-153 pattern: reproduce a committed value before emitting.
    a1-genesis.json is frozen and was itself generated independently."""
    a1 = json.load(open(HERE / "a1-genesis.json"))
    assert OWNER_PUB.hex() == a1["owner_kex_pub_hex"], "A1 owner_kex cross-check failed"
    assert multibase_x(OWNER_PUB) == a1["owner_kex_pub_multibase"], "A1 multibase failed"
    a2 = json.load(open(HERE / "a2-did.json"))
    # keys.kex of the DID document IS what §03.1 makes the owner line's kid
    assert a1["owner_kex_pub_multibase"] in a2["did_doc_jcs"], "A2 kex pin failed"
    return a2["did"]


DID = crosscheck_a1()


# --- C1 + C2: reconstruct and assert, never rewrite --------------------------

def check_c1() -> None:
    """Settles CHDR-025: the file claims independent generation; this proves it."""
    committed = json.load(open(HERE / "c1-header-seal.json"))
    aad = line_aad(DID, NODE, KEY_VERSION)

    assert OWNER_SK.hex() == committed["owner_kex_sk_hex"]
    assert OWNER_PUB.hex() == committed["owner_pub_hex"]
    assert STRANGER_PUB.hex() == committed["grantee_pub_hex"]

    for name, esk, nonce, recipient_pub in (
        ("owner_line", C1_OWNER_ESK, C1_OWNER_N, OWNER_PUB),
        ("grantee_line", C1_GRANTEE_ESK, C1_GRANTEE_N, STRANGER_PUB),
    ):
        epk, c = seal_line(esk, recipient_pub, DK, nonce, aad)
        assert epk.hex() == committed[name]["epk_hex"], f"C1 {name} epk drift"
        assert c.hex() == committed[name]["c_hex"], f"C1 {name} ciphertext drift"

    w = committed["wrap"]
    c = seal_wrap(
        bytes.fromhex(w["via_key_hex"]),
        bytes.fromhex(w["dk_hex"]),
        bytes.fromhex(w["n_hex"]),
        wrap_aad(DID, w["wrapped_node"], w["key_version"]),
    )
    assert c.hex() == w["c_hex"], "C2 wrap drift"
    print("verified c1-header-seal.json (C1+C2) — reconstructed, not rewritten")


# --- C3: the I3 owner-line cases --------------------------------------------

def header(lines: list) -> dict:
    """spec 03.1 object shape."""
    return {"object": "header", "v": 1, "node": NODE,
            "key_versions": {str(KEY_VERSION): {"lines": lines}}}


def gen_c3() -> dict:
    aad = line_aad(DID, NODE, KEY_VERSION)
    owner_kid = multibase_x(OWNER_PUB)          # §03.1 variant A
    stranger_kid = multibase_x(STRANGER_PUB)

    # The positive owner line is byte-identical to c1-header-seal.json's:
    # same esk, nonce, dk, node, version, recipient.
    o_epk, o_c = seal_line(C1_OWNER_ESK, OWNER_PUB, DK, C1_OWNER_N, aad)
    owner_line = line("owner", owner_kid, o_epk, C1_OWNER_N, o_c)

    s_epk, s_c = seal_line(C3_ESK["stranger"], STRANGER_PUB, DK, C3_NONCE["stranger"], aad)
    stranger_line = line(stranger_kid, stranger_kid, s_epk, C3_NONCE["stranger"], s_c)

    # negative 2: to says "owner", declared kid is the stranger's, sealed to the
    # stranger. Keyless verifiers catch it: no line declares owner_kex.
    fa_epk, fa_c = seal_line(C3_ESK["foreign_a"], STRANGER_PUB, DK, C3_NONCE["foreign_a"], aad)
    foreign_key_line = line("owner", stranger_kid, fa_epk, C3_NONCE["foreign_a"], fa_c)

    # negative 3: declared kid IS owner_kex, seal is to the stranger. Only a
    # verifier holding owner_kex catches it — the documented boundary of §1.3.
    fb_epk, fb_c = seal_line(C3_ESK["foreign_b"], STRANGER_PUB, DK, C3_NONCE["foreign_b"], aad)
    foreign_seal_line = line("owner", owner_kid, fb_epk, C3_NONCE["foreign_b"], fb_c)

    # positive 2: the label points elsewhere, the seal is the owner's.
    u_epk, u_c = seal_line(C3_ESK["unlabelled"], OWNER_PUB, DK, C3_NONCE["unlabelled"], aad)
    unlabelled_line = line(stranger_kid, owner_kid, u_epk, C3_NONCE["unlabelled"], u_c)

    cases = [
        {"name": "owner_line_present", "verdict": "valid", "tier": "keyless",
         "proves": "…", "header": header([owner_line, stranger_line])},
        {"name": "no_owner_line_at_all", "verdict": "invalid",
         "must_fail": "MissingOwnerLine", "tier": "keyless",
         "proves": "…", "header": header([stranger_line])},
        {"name": "owner_label_foreign_key", "verdict": "invalid",
         "must_fail": "MissingOwnerLine", "tier": "keyless",
         "proves": "…", "header": header([foreign_key_line])},
        {"name": "owner_label_foreign_seal", "verdict": "invalid",
         "must_fail": "MissingOwnerLine", "tier": "owner_kex",
         "proves": "…", "header": header([foreign_seal_line])},
        {"name": "unlabelled_owner_line", "verdict": "valid", "tier": "keyless",
         "proves": "…", "header": header([unlabelled_line])},
    ]

    return {
        "vector": "C3",
        "description": "…",                     # see §3.3 of the proposal
        "seed_hex": SEED.hex(),
        "subject_did": DID,
        "node": NODE,
        "key_version": KEY_VERSION,
        "dk_hex": DK.hex(),
        "owner_kex_sk_hex": OWNER_SK.hex(),
        "owner_kex_pub_hex": OWNER_PUB.hex(),
        "owner_kex_pub_multibase": owner_kid,
        "stranger_sk_hex": STRANGER_SK.hex(),
        "stranger_pub_hex": STRANGER_PUB.hex(),
        "stranger_multibase": stranger_kid,
        "cases": cases,
    }


def encoded(vector: dict) -> bytes:
    return (json.dumps(vector, indent=2, ensure_ascii=False) + "\n").encode()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--output", type=Path, default=HERE / "c3-owner-line.json")
    args = parser.parse_args()

    check_c1()                                  # frozen: assert, never write
    payload = encoded(gen_c3())
    if args.check:
        if not args.output.exists():
            raise SystemExit(f"missing {args.output}")
        if args.output.read_bytes() != payload:
            raise SystemExit(f"{args.output} is not reproducible")
        print(f"verified {args.output}")
        return
    args.output.write_bytes(payload)
    print(f"wrote {args.output}")


if __name__ == "__main__":
    main()
```

**Ce qui reste à faire au propriétaire :** confirmer les rampes d'octets de
`C3_ESK` / `C3_NONCE` ; remplir les `description` et `proves` (§3.3 en donne le
texte) ; ajouter les deux entrées à `vectors/ownership.json` ; écrire le
consommateur Rust, dont **le cas d'édition** — sans lui, C3 regate ce qui l'était
déjà.

**Avertissement de provenance, à lire avant d'exécuter.** Ce squelette n'a
**jamais été exécuté** : aucune commande n'a été lancée pour produire ce
document. Les primitives sont transcrites de `spec/03-headers.md:119-139` et de
`rust/crates/aithos-core/src/seal.rs:15-49`, et le fait que
`c1-header-seal.json.owner_pub_hex` et `a1-genesis.json.owner_kex_pub_hex`
portent la même valeur `2a87b432…834165` est établi **par lecture**, pas par
calcul. Si `check_c1()` échoue au premier lancement, la première hypothèse à
tester n'est pas le vecteur mais ce squelette — en particulier le sel du HKDF
(`salt=b""` → bloc de zéros de la taille du hash, RFC 5869 §2.2) et l'ordre
`epk ‖ recipient_pub` dans l'`info`.

---

## 5. Impact sur les implémentations

**Ce qui devient invalide de ce qui était valide.**

1. Une édition dont un `header.json` pinné a une `key_version` sans ligne owner :
   **acceptée aujourd'hui**, refusée demain. Aucun vérificateur ne la refuse à
   cette révision — ni `Bundle::verify` (`bundle.rs:1654-1769`), ni
   `publication::cold_verify` (`publication.rs:836-939`).
2. Un header dont la ligne étiquetée `"owner"` scelle vers une clé qui n'est pas
   l'`owner_kex` du sujet : **accepté aujourd'hui** par les quatre points de
   contrôle I3 (`header.rs:71-77`, `:298-303`, `:310`), refusé demain.
3. Sous la variante A seulement : tout header dont la ligne owner porte
   `kid: "owner-kex"` devient non conforme au fil. C'est le coût de la variante,
   et il est mesurable : il touche `Recipient::owner` (`header.rs:22-28`),
   `vectors/g2-rotation.json` (`old_kids: ["owner-kex", …]`, gelé), et
   `spec/03-headers.md:20`.

**Ce qu'un implémenteur tiers doit changer.**

- Un **lecteur** (niveau *Core reader*, §9.4) : il doit désormais désérialiser
  chaque `header.json` que l'édition épingle en objet typé et le valider, là où
  il pouvait se contenter d'un digest opaque. Le coût est faible et la décision
  le documente : le chemin de vérification énumère et parse **déjà** chaque
  header pour calculer `BLAKE3(JCS(…))` (`state.rs:58-68`) ; ce qui manque est un
  parse typé et un appel, pas une énumération nouvelle.
- Un **écrivain** : il doit lire `keys.kex` du document DID pour construire ou
  re-sceller une ligne owner, au lieu de poser un littéral. Neuf des dix appels
  de production recensés par la décision ont déjà la clé en portée
  (`owner_kex_recipient()` — `grants.rs:171-174` — ou `owner.owner_kex_pub()`).
- Un **rotateur** (délégué inclus, `spec/05-delegation.md:85-88`) : il ne peut
  plus reconduire la ligne owner qu'il trouve ; il doit la reconstruire depuis le
  document DID. Trois sites de production font déjà le bon geste sur le mauvais
  critère — `revoke.rs:180`, `structure.rs:259`, `vault.rs:375` remplacent toute
  ligne dont `line.to == "owner"` par `owner_kex_recipient()`.
- Une **surface de fabrication de header** : elle ne doit plus pouvoir produire
  silencieusement un header que `verify` rejetterait. `aithos-cli`
  (`rust/crates/aithos-cli/src/cmd/header_seal.rs:30-44`) accepte aujourd'hui des
  destinataires au format libre `label:kid:x25519_pub_hex` et les passe tels
  quels à `Header::build` (`:56`), en se déclarant « DEV surface over test keys »
  (`:1-2`) — sa doc d'argument dit « one MUST be labelled "owner" » (`:14`),
  c'est-à-dire exactement le critère que la décision écarte. La décision impose
  la contrainte au correcteur et lui laisse le choix des deux moyens.

**Ce qui ne change pas.** Aucun octet signé. Aucune AAD. Aucun profil de mandat.
Aucune primitive de §00.3 ou §03.8. La compatibilité des ciphertexts existants
est intacte : ce qui change est ce qu'un vérificateur **accepte**, jamais ce
qu'un producteur **calcule**.

---

## 6. Ce que cette proposition ne fait pas

**`check_rotation` : inclusion là où la spec exige une égalité.**
`spec/03-headers.md:93-96` dit « the new version's lines MUST **equal** the
previous lines minus the revoked ». `Header::check_rotation` (`header.rs:274-303`)
teste une **inclusion** : il construit `prev_kids` et rejette toute ligne
nouvelle dont le `kid` est absent, mais ne vérifie jamais qu'aucune ligne
survivante n'a disparu. Une rotation qui supprime un survivant passe. C'est
`CHDR-024`, que la décision consigne explicitement sans l'assigner, et que ce
document n'amende pas.

**Reste-t-il séparable une fois `SI3-1` à `SI3-10` posés ? Oui.** Les deux
défauts portent sur des ensembles disjoints : `SI3-4` traite l'élément distingué
qu'est la ligne owner, `CHDR-024` traite le cardinal de l'ensemble des
survivants. Un `check_rotation` corrigé sur l'owner reste faux sur les survivants,
et réciproquement ; aucune des deux corrections n'a besoin de l'autre.

**Une précaution a néanmoins été prise.** `SI3-4` réécrit la phrase de
`spec/03-headers.md:93-96` — celle-là même que `CHDR-024` cite. La proposition
« the new version's lines MUST equal the previous lines minus the revoked » y est
donc reprise **verbatim**, sans une virgule de changement, et l'ajout est fait
en apposition. Le futur amendement d'égalité trouvera son ancre intacte.

**Deux autres choses laissées de côté, délibérément.**

- **L'autorité du signataire d'une rotation.** `spec/05-delegation.md:97-99`
  l'énonce déjà (« A verifier rejects a header rotation whose signer is not an
  authorized issuer… ») et `docs/proposals/header-rotation-authority.md` propose
  de l'implémenter, au statut *Proposé — non adopté*. `SI3-5` s'appuie sur ce
  passage sans le modifier : il n'entre pas dans le champ de la décision, et sa
  proposition a son propre décideur, « à désigner ».
- **La forme du contrôle dans `state.rs`.** Validation en ligne dans
  `header_hash_at` ou passe dédiée : la décision la laisse au correcteur, et ce
  document n'y touche pas.

---

## 7. Amendements suggérés, hors décision

Cette section est détachable. Le propriétaire peut la refuser entièrement sans
toucher à §2.

### `HD-1` — `docs/CONFORMANCE.md:50-52`

> All vectors are generated by an independent Python implementation
> (`vectors/gen-*.py`, blake3 + PyNaCl + base58) and frozen once green

Cette phrase est fausse à la révision `0148ea5` : aucun `gen-c*` n'existe, et la
famille C est listée comme couverte dans la table du même document. Elle redevient
vraie dès que `gen-c.py` (§4) est écrit. Si le propriétaire préfère ne pas écrire
le générateur, c'est cette phrase et la `description` de `c1-header-seal.json`
qu'il faut corriger — et la seconde est gelée, ce qui exigerait un nouvel id de
vecteur. La voie du générateur est la moins coûteuse des deux.

### `HD-2` — un vecteur négatif pour `spec/05-delegation.md:97-99`

`vectors/g2-rotation.json` porte trois assertions — `expected_survivor_kids`,
`smuggled_must_fail`, `missing_owner_must_fail` — et aucun cas d'autorité, alors
que §5.5 énonce l'obligation depuis toujours. Hors décision, mais c'est la
prochaine chose qu'un implémenteur tiers découvrira.

### `HD-3` — contrôle positif dans `c1_fail_closed`

`rust/crates/aithos-core/tests/c1_header_seal.rs:83-108` porte quatre assertions
négatives et **aucun contrôle positif dans son propre corps** : que le triplet non
modifié s'ouvre sous l'AAD nominale n'est établi que dans une autre fonction
(`:76-81`). Toute mutation de `line_aad` changerait l'AAD des deux côtés à la
fois. C'est la première moitié du critère de clôture de `CHDR-025`, dont §4 ne
traite que la seconde. Une ligne à ajouter, mais elle relève du lot de
correction, pas du lot de spécification.

---

## 8. Contradictions résiduelles relevées en chemin

Consignées, non amendées, pour qu'elles ne soient pas découvertes par un
implémenteur.

1. **I3 par la clé n'a pas de témoin sans clé** (§1.3). C'est la plus lourde :
   elle contraint la rédaction de `SI3-2` et impose au propriétaire un arbitrage
   A/B que la décision n'avait pas anticipé.
2. **Le bandeau de version est périmé** : `spec/00-overview.md:3` annonce
   `"1.0.0-draft.1"` alors que `spec/00-overview.md:70` et
   `spec/02-content-tree.md:226` traitent `"1.0.0-draft.2"` comme profil
   d'émission courant. Réparé par `SI3-10`.
3. **Le modèle de profils de §0.4 ne sait pas exprimer un durcissement rétroactif
   de vérification.** Ses deux profils « introduisent » des constructions signées.
   `SI3-10` ajoute la règle manquante plutôt que de forcer le durcissement dans un
   moule qui ne le contient pas.
4. **`spec/03-headers.md:42` autorise un lecteur à se reconnaître par `"owner"`**,
   dernière trace normative d'une identification par étiquette. Traité par
   `SI3-3`.
5. **`spec/06-revocation.md:33` écrit « + owner »** dans le pseudo-code de la
   procédure de révocation, là où il faut une clé. Traité par `SI3-6`.
6. **`aithos-cli/src/cmd/header_seal.rs:14`** documente `--recipient` par
   « one MUST be labelled "owner" » : une surface publique qui enseigne le critère
   que la décision écarte. Relève du lot B, signalé ici parce que c'est du texte
   destiné à un utilisateur.
