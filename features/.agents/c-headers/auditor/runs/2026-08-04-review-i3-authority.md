# Conclusion — revue indépendante de la correction lot B `c-headers` : I3 authority

| Champ | Valeur |
|---|---|
| Type de run | revue de correction, deux passes, native (**pas** `RECONSTRUCTED`) |
| Rôle | `audit-c-headers` en position R1 — reviewer indépendant |
| Date | 2026-08-04 |
| Révision auditée à l'origine | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` |
| Baseline de correction | `5be3047a0665d6d6415ec263bd95e044be04c15a` (`rust/` byte-identique à `a2087f2`) |
| Révision candidate | `9dc58895b5c822d13ea5daf8c25302ccd657b668`, branche `codex/fix-c-headers-i3-authority` |
| Journal d'orchestration | `../../../orchestrator/runs/2026-08-04-r2/` (revue) ; `../../../orchestrator/runs/2026-08-04-r1/` (correction) |
| Périmètre | `CHDR-007` et `CHDR-012` **seuls**. Les neuf findings du lot A ne sont pas jugés |
| Unités de revue | RU-A — I3 à l'échelle de l'édition (`CHDR-007`) ; RU-B — le champ définitoire de la ligne owner (`CHDR-012`) |
| Findings traités | `CHDR-007` → **`VERIFIED`** ; `CHDR-012` → **`VERIFIED`** |
| Findings ouverts par cette revue | `CHDR-028` (P2, `disclosure: embargo`), `CHDR-029` (P2), `CHDR-030` à `CHDR-036` (P3) — **aucun assigné** |
| Findings non traités | lot A (`CHDR-001`, `-002`, `-009`, `-013`, `-014`, `-016`, `-019`, `-021`, `-025`) ; `CHDR-024`, `-026`, `-027` |
| État du worktree en fin de run | 2 fichiers modifiés — `docs/audits/features/c-headers.md`, `features/c-headers.feature` — plus ce rapport. Aucun commit. `rust/`, `spec/`, `vectors/`, `STATE.md`, `PROCESS.md`, `QUEUE.yaml`, la décision et le rapport du correcteur : **intouchés** |
| Résultat | `REVIEW_ACCEPTED` sur les deux findings assignés. Une condition de blocage est ouverte (§8) |

## 1. Modèle à deux passes — ce qui a été gelé, et quand

**Pass A — aveugle à l'histoire.** Conduit sur un extrait `git archive` de
`9dc5889` **sans `.git`** et **sans le rapport du correcteur**, sous
`runs/2026-08-04-r2/passA/c-headers/REVIEW-PASSA/`. Une seule preuve
d'exécution était disponible : `ev-dc461083` (gate `@c-headers`, 1/4/8/28).
Aucune commande `cargo`, aucun build, aucun `git`.

Le verdict provisoire a été **gelé avant toute livraison de Pass B** :
`pass-a/review-frozen.json`, sha256 `50f080fb…`, journalisé par
l'orchestrateur, extrait purgé ensuite. **`contamination: none`** au moment du
gel.

**Pass B.** Dépôt complet, `git log` / `git diff` / `git show` autorisés, seize
transcripts du run `2026-08-04-r1`, le rapport du correcteur, l'étalon de
juillet `af32734`. Toujours aucune exécution : les seize gates ont été possédés
par l'orchestrateur et sont cités par `evidence_id`.

Ce rapport ne revendique pas d'avoir exécuté quoi que ce soit. Il distingue
partout les faits établis **par lecture de l'arbre** (`git diff`, `grep`,
recalcul de digests) de ceux **rapportés par transcript**.

**Vérification indépendante conduite en propre, hors dépôt.** Au Pass A, les
cinq cas de `vectors/c3-owner-line.json` ont été **rouverts** par une
réimplémentation Python du primitif de ligne (X25519 → HKDF-SHA256 sur
`aithos-core/v1/hdr-kek ‖ 0 ‖ epk ‖ recipient_pub` → XChaCha20-Poly1305, AAD
`aithos-core/v1/header-line ‖ 0 ‖ did ‖ 0 ‖ node ‖ 0 ‖ version`), écrite dans
`/tmp` et sans lire l'implémentation Python du dépôt. Résultat : les cinq
verdicts et les deux paliers du vecteur sont cryptographiquement exacts, et
discriminent dans les deux directions. C'est le seul oracle de cette revue qui
ne dépende ni du code Rust ni du générateur du dépôt.

| Cas C3 | `kid == owner_kex` | ouvre sous `owner_kex` | verdict du vecteur |
|---|---|---|---|
| `owner_line_present` | oui | **oui** | valid / keyless |
| `no_owner_line_at_all` | **non** | non | invalid / keyless |
| `owner_label_foreign_key` | **non** | non | invalid / keyless |
| `owner_label_foreign_seal` | oui | **non** | invalid / `owner_kex` |
| `unlabelled_owner_line` | oui | **oui** | valid / keyless |

Les digests épinglés ont été recalculés : `c3-owner-line.json`
`2686d3ab…`, `c1-header-seal.json` `af0f63bd…`, `g2-rotation.json`
`be223ff1…` — tous concordants avec `vectors/ownership.json`.

## 2. Réconciliation Pass A / Pass B, finding par finding

### `CHDR-007` — I3 à l'échelle de l'édition

**Verdict Pass A gelé.** `closure_criterion_met: true`. Établi par lecture
seule : `bundle::verify_pinned_headers` (`bundle.rs:302-320`) parse en type
`Header` chaque objet épinglé reconnu par `is_header_file` (`:291-295`) et
appelle `validate(doc.keys.kex)` ; l'appel est réel dans `Bundle::verify`
(`:1759`) et dans `publication::cold_verify` (`publication.rs:897`). Le filtre
avait été vérifié exhaustif contre **tous** les sites d'écriture de header du
dépôt et contre la grammaire fermée `validate_store_key` (`lib.rs:142-224`) :
ni faux négatif, ni faux positif. Marqué **non vérifiable** : que le test
échoue sur la baseline pour la raison nommée.

**Preuve différentielle apportée par le Pass B.** `ev-47ec8aac`, baseline
`5be3047`, `c3_owner_line_edition` : **0 passed / 3 failed**. Les deux tests
d'invalidité échouent par

```
panicked at c3_owner_line_edition.rs:244:10:
an edition pinning a header with no owner line is invalid (I3, §09.4): ()
```

— c'est-à-dire que `expect_err` reçoit `Ok(())`. **`Bundle::verify` acceptait
l'édition mutilée.** C'est exactement, et uniquement, la raison nommée par le
critère de clôture ; l'échec n'est ni un panic de fixture, ni une erreur de
compilation, ni un message parasite. Le troisième échec
(`:213`, `left: ["owner-kex"]`) relève de `CHDR-012`.

GREEN correspondant : `ev-b925a0cf`, 3/3. Contexte : `ev-2b8ccdc0` (feature
1/4/8/28), `ev-03c0fdfc` (Cucumber global 18/114/836/3577), `ev-8bfeccca`
(workspace complet, 0 `FAILED` — recompté sur le transcript), `ev-e3b0c442`
(`fmt --check`).

**Verdict réconcilié : `VERIFIED`. Il n'a pas changé.** Le Pass B n'a rien
retiré ni ajouté au raisonnement ; il a converti en fait observé la seule
inférence que le Pass A avait dû laisser ouverte. C'est la valeur propre de la
barrière : le verdict de code a été atteint sans la preuve, puis la preuve l'a
confirmé au lieu de le produire.

### `CHDR-012` — le champ définitoire de I3

**Verdict Pass A gelé.** `closure_criterion_met: true`. `check_owner_line`
(`header.rs:84-94`) compare `r.pubkey.as_bytes() == owner_kex.as_bytes() &&
r.kid == kid` ; `validate` (`:371`) et `check_rotation` (`:334`, `:357`)
reçoivent le kid owner attendu ; `Recipient::owner` (`:35`) nomme sa clé. Les
neuf sites d'écriture internes passent la clé lue dans `did.json` via
`owner_kex_pub()` (`grants.rs:176-179`). Aucun `OWNER_LABEL` ni `l.to` ne
subsiste dans un contrôle I3 de production — vérifié par grep exhaustif sur
`crates/*/src/`.

**Preuve différentielle apportée par le Pass B.** `ev-15f8f483`, baseline,
`c3_owner_line` : 2 passed / 3 failed, avec l'écart reproduit **dans les deux
directions** :

```
left: ["owner_label_foreign_key: keyless I3 must reject (verdict=invalid,
        tier=keyless), accepted",
       "unlabelled_owner_line: keyless I3 must accept (verdict=valid,
        tier=keyless), rejected: I3 violated — header without an owner line"]
```

et `Recipient::owner` rendant `kid: "owner-kex"` là où §3.1 exige
`z6LSeYCJg2G3i6zEiYd2bvnacfR8EnQoUUv3315nBbJL85sS`. Un header étiqueté
`"owner"` mais scellé ailleurs était **accepté** ; un header scellé à
`owner_kex` mais étiqueté autrement était **rejeté**. C'est l'énoncé du finding,
mot pour mot, et rien d'autre.

GREEN : `ev-9f82e070` 6/6, `ev-b925a0cf` 3/3, `ev-f4579eab` 4/4,
`ev-b19b0db3` 3/3 inchangé, `ev-6608a56c`, `ev-fa196226`, `ev-6469eead`,
`ev-88c136d4`.

**Fait de premier ordre, établi par exécution et non par lecture.** Sur
`c3_positive_owner_line_is_byte_exact` (`ev-15f8f483`), la ligne construite et
la ligne attendue diffèrent **par le seul `kid`** — `epk`, `n` et `c`
identiques caractère par caractère. La variante A ne redérive donc **aucun
chiffré** : `kid` est absent de l'AAD de ligne. C'est la raison pour laquelle
`c1-header-seal.json` et son pin n'ont pas bougé, et c'est le fait porteur du
lot. Le Pass A l'avait inféré du code de `seal.rs` ; le transcript l'établit.

**Verdict réconcilié : `VERIFIED`. Il n'a pas changé.**

### Ce que le Pass A avait mal calibré

Un point, et il faut le dire. Le Pass A a qualifié `g2_rotation.rs`
d'`overstated` au motif que son `G2_OWNER_KID` est le littéral synthétique
`"owner-kex"`. Le Pass B révèle que ce n'est pas une négligence mais une
**décision instruite, contestée, tranchée contre le rôle qui l'avait
recommandée, puis démentie par un gate et re-tranchée** (§`g2-rotation.json`
du rapport du correcteur ; le rouge `ev-8eab8e17` sur
`cb2_bundle_structure_vault_historical_hashes_preexisting_green`,
`cb2_bundle_structure_vault.rs:133`, non effacé par `ev-8bfeccca`). La
qualification technique reste juste — la branche `MissingOwnerLine` de
`check_rotation` (`header.rs:357`) n'est exercée par aucun test — mais le
jugement de méthode qu'elle portait était injuste. Consigné.

### Une inexactitude du rapport du correcteur

`PROCESS.md` traite ce rapport comme une revendication à vérifier. Trois
affirmations ont été recoupées et **tiennent** : `vectors/` byte-identique à
`5be3047` (`git diff --stat 5be3047..9dc5889 -- vectors/` vide) ; aucune ligne
commençant par `assert`, `panic!` ou `expect(` déplacée dans `cucumber.rs`
(`git diff … | grep` vide) ; workspace sans `FAILED` sur `ev-8bfeccca`.

Une **ne tient pas** : « No `"owner-kex"` literal remains in the repository ».
Le littéral subsiste en `vectors/gen-g.py:103`, `vectors/g2-rotation.json:6` et
`:12`, `rust/crates/aithos-core/tests/g2_rotation.rs:21`. C'est la conséquence
directe et assumée de l'option (c′) que la section précédente du même rapport
décrit longuement : la phrase se contredit avec son propre récit. **Le code de
production, lui, en est bien exempt** (grep sur `crates/*/src/` : zéro).
Surdimensionnement de rédaction, sans effet sur le verdict.

## 3. Arbitrage explicite des trois réserves du Pass A

La question posée à chacune : **empêche-t-elle le `VERIFIED`, ou vit-elle sa
propre vie ?**

**Réserve 1 — le troisième vérificateur d'édition.** C'est le cas dur, et je
l'assume dans les deux sens. Il **n'empêche pas** `CHDR-007` d'être `VERIFIED`,
et devient `CHDR-028`, sous embargo (§8). Trois motifs. *Un* : le critère de
clôture nomme deux fonctions, la décision une seule ; les deux sont faites et
prouvées par RED → GREEN. *Deux* : la surface visée vérifie un **candidat**
contre un contexte de vérification, à un moment de vie antérieur à l'édition
publiée, et la forme résidente en Store du même paquet est bel et bien gatée par
`cold_verify` — ce n'est donc pas le même objet vérifié deux fois avec deux
verdicts. *Trois* : elle a sa propre reachability et mérite son propre critère
de clôture, pas une réouverture qui renverrait le correcteur sur un travail
déjà juste. **Contre-argument que je ne cache pas** :
`spec/09-cli-and-conformance.md` §9.4 dit « on every `aithos-core` manifest
profile », et le correcteur a lui-même invoqué cette clause pour inclure
`cold_verify` que la décision ne nommait pas. Le même raisonnement, poussé d'un
cran, atteindrait cette surface. La lecture qui l'emporte appartient au
propriétaire, pas au reviewer ; je pose le verdict et je lui remets le levier.

**Réserve 2 — la résolution de clé depuis `line.to`.** N'empêche pas
`CHDR-012` d'être `VERIFIED` ; devient `CHDR-029`, P2. Motif décisif, établi
par le Pass B et impossible au Pass A : `git diff 5be3047..9dc5889` **ne touche
pas** ces quatre lignes. Seules les branches `line.to == "owner"` voisines ont
été retirées. Le défaut **préexiste** à la correction, n'en est pas une
régression, et porte sur des résolutions de clé de grantee, non sur les trois
points de contrôle I3 que le critère nomme.

**Réserve 3 — le palier `owner_kex` mort en production.** N'empêche pas ;
devient `CHDR-030`, P3. `validate_as_owner` est une **addition** de la
correction (absent de `5be3047`, vérifié par `git show`). Le correcteur a livré
plus que le critère ne demandait, sans câbler l'appelant. Aucun des deux
critères de clôture ne mentionne ce palier : c'est une dette de conformité
contre la spec amendée par le lot `SI3-*`, pas un défaut du correctif.

## 4. Verdicts

| Finding | Sév. | Verdict | Motif en une phrase |
|---|---|---|---|
| `CHDR-007` | P1 | **`VERIFIED`** | Les deux vérificateurs d'édition nommés par le critère parsent chaque header épinglé et rejettent l'édition sans détenir de clé ; RED `ev-47ec8aac` (`Ok(())` sur la baseline) → GREEN `ev-b925a0cf`, échec baseline **pour la raison nommée** |
| `CHDR-012` | P2 | **`VERIFIED`** | I3 est décidé sur la clé destinataire et sur le `kid` qui la nomme, plus jamais sur `to` ; RED `ev-15f8f483` reproduisant l'écart **dans les deux directions** → GREEN `ev-9f82e070` 6/6, et l'oracle C3 redérivé indépendamment confirme la sémantique |

Aucun rejet. Le compteur de rejets de ces deux findings reste à **0 sur 3**.

## 5. Commandes et résultats

Cette revue **n'a exécuté aucune commande de build ou de test**. Les résultats
cités sont ceux des seize gates possédés par l'orchestrateur au run
`2026-08-04-r1`, plus `ev-dc461083` au run `2026-08-04-r2`.

| `evidence_id` | Rôle dans ce verdict |
|---|---|
| `ev-dc461083` | seule preuve du Pass A — gate `@c-headers`, 1/4/8/28 |
| `ev-15f8f483` | **RED** baseline `c3_owner_line`, 2/3 — preuve de `CHDR-012` |
| `ev-47ec8aac` | **RED** baseline `c3_owner_line_edition`, 0/3 — preuve de `CHDR-007` |
| `ev-9f82e070` | GREEN `c3_owner_line` 6/6 |
| `ev-b925a0cf` | GREEN `c3_owner_line_edition` 3/3 |
| `ev-b19b0db3` | GREEN `c1_header_seal` 3/3, inchangé — non-régression du pin byte-exact |
| `ev-f4579eab` | GREEN `g2_rotation` 4/4 après retour à (c′) |
| `ev-6608a56c`, `ev-6469eead` | GREEN pins de vecteurs, `cb10` |
| `ev-fa196226` | GREEN régressions core c1/g2/g3/b2 |
| `ev-88c136d4` | GREEN contrôle direct de `ev-8eab8e17` |
| `ev-2b8ccdc0` | GREEN feature `@c-headers` 1/4/8/28 |
| `ev-03c0fdfc`, `ev-8d23a708` | GREEN Cucumber global 18/114/836/3577 |
| `ev-8bfeccca` | GREEN workspace complet, 836/3577, 0 `FAILED` |
| `ev-8eab8e17` | **RED** workspace antérieur — consigné, non effacé ; c'est lui qui a démenti l'arbitrage (a) sur `g2-rotation.json` |
| `ev-e3b0c442` | GREEN `cargo fmt --check` |

Vérifications conduites en propre, par lecture ou calcul, sans exécution du
dépôt : recalcul des trois sha256 de vecteurs ; redérivation cryptographique des
cinq cas C3 hors dépôt ; `git diff`/`git show` sur les quatre affirmations du
rapport du correcteur ; comptage statique du contrat de feature après édition
(4 `Rule` / 8 `Scenario` / 28 pas).

## 6. Fichiers et symboles affectés — par la revue

- `docs/audits/features/c-headers.md` — bandeaux de clôture sur `CHDR-007` et
  `CHDR-012` (énoncés d'origine **conservés** : ils décrivent le code audité) ;
  nouvelle **§6bis** portant `CHDR-028` à `CHDR-036`.
- `features/c-headers.feature:47-51` — retrait de `@chdr-007 @chdr-012` et des
  quatre lignes de commentaire adjacentes, conformément à `PROCESS.md`
  § *Gherkin audit-marker lifecycle*. `@audit-partial @chdr-009 @chdr-011
  @chdr-010` et leur explication sont **conservés** : lot A, non résolu. Aucun
  marqueur ajouté — les neuf findings nouveaux sont `OPEN` et aucun n'est un
  défaut de scénario.
- Ce rapport.

## 7. Surfaces, formats et symboles affectés — par la correction revue

Établi par lecture du diff, pour la revue d'impact.

- **`aithos-core`** — rupture d'API : `Header::build`, `build_at`, `rotate`
  prennent `owner_kex: &XPublicKey` ; `validate` et `check_rotation` prennent
  `owner_kid: &str`. Ajouts : `owner_kid()`, `validate_as_owner()`,
  `open_owner()`, `open_owner_latest()`.
- **Format au repos** — rupture rétroactive : un header écrit par un binaire
  antérieur porte `kid: "owner-kex"` et échoue désormais `Bundle::verify`.
  Aucun artefact de ce type dans l'arbre ; l'obligation a été arbitrée comme
  rétroactive par le propriétaire. Relève de la revue d'impact et de
  `CHDR-033`.
- **`aithos-bundle`** — `is_header_file`, `verify_pinned_headers`, l'appel dans
  `Bundle::verify` et dans `cold_verify` ; `owner_kex_pub()` /
  `owner_kex_recipient()` ; treize sites de lecture migrés vers `open_owner*`.
- **`aithos-cli`** — `header-seal` exige `--owner-kex-hex` ; `header-open`
  exige `--owner-kid`. Rupture de ligne de commande.
- **`aithos-wasm`, `aithos-owner`** — aucun contact avec les headers, vérifié
  par grep. Non impactés.
- **Vecteurs** — aucun. `vectors/` byte-identique à `5be3047`.
- **Revue d'impact due**, non conduite ici (geste humain) : `g-revocation`,
  `d-bundle`, `n-structural-mutations`, `o-connector-classes-vault`.

## 8. Blocage identifié — appartient à l'orchestrateur

**Barrière de divulgation sur `CHDR-028`.** `aithos-core` est public et cette
branche y sera poussée. L'énoncé complet de `CHDR-028` décrirait un chemin
actuellement **non corrigé** et **non assigné** par lequel une édition non
conforme à I3 obtient un verdict d'acceptation sur une API publique consommée
par un plan de publication. Le producteur d'un tel objet n'est pas nécessairement
le sujet lui-même : `spec/05-delegation.md:85-91` autorise un délégué ou un
ancêtre à republier des headers, et c'est le raisonnement même par lequel
l'audit a écarté la défense « ce n'est que de l'auto-sabotage » pour
`CHDR-012`.

L'audit public ne porte donc que l'**identifiant et un titre neutre**. Le texte
intégral — énoncé, chemin d'appel, preuves `fichier:ligne`, critère de clôture —
est transmis hors dépôt à l'orchestrateur. **La levée appartient au propriétaire
du protocole**, comme le 2026-08-03 pour `CHDR-007` et `CHDR-012`.

`CHDR-029`, également P2, est publié **en entier** : sa précondition — une ligne
dont `to` et `kid` divergent — ne peut être produite par aucun écrivain de
production, ce qui la borne à un header édité à la main ou importé. La
différence de traitement est délibérée et argumentée dans chaque bloc.

## 9. Limites de cette conclusion

- **Cette revue n'a exécuté aucune commande.** Elle ne peut attester que les
  seize gates ont tourné ; elle atteste qu'aucun résultat n'est revendiqué sans
  `evidence_id`, et que les transcripts cités disent bien ce qui leur est fait
  dire — vérifié ligne à ligne pour les deux RED.
- Le `VERIFIED` porte sur **le comportement du code candidat au regard des deux
  critères de clôture**, pas sur l'absence de tout défaut dans les surfaces
  touchées. Neuf findings nouveaux en témoignent.
- La barrière du Pass A protège la partie *lecture de code* du verdict, pas la
  partie *preuve d'exécution* : celle-ci vient entièrement du Pass B et donc du
  correcteur et de l'orchestrateur. Le seul oracle réellement indépendant de la
  chaîne est la redérivation Python des cinq cas C3 (§1).
- `CHDR-024` reste ouvert et non assigné, comme la décision l'instruit :
  `check_rotation` est une inclusion là où `spec/03-headers.md:93-96` exige une
  égalité.
- La revue d'impact sur les quatre features voisines reste due et n'est pas
  couverte ici.
- `CHDR-028` étant sous embargo, la présente conclusion est **incomplète en
  lecture publique** tant que le propriétaire n'a pas statué.

## 10. Action suivante et compétence attendue

1. **Orchestrateur** — relancer le gate de feature après l'édition des
   marqueurs, pour confirmer par exécution le contrat 1 feature / 4 `Rule` /
   8 `Scenario` / 28 pas que cette revue n'a vérifié que statiquement ; porter
   la condition de blocage `CHDR-028` au propriétaire ; passer `STATE.md` à
   `REVIEW_ACCEPTED` pour le lot B.
2. **Propriétaire du protocole** — statuer sur la divulgation de `CHDR-028`, et
   sur la lecture de §9.4 qui décide si la surface qu'il vise relève de la même
   obligation que `Bundle::verify` et `cold_verify`. Statuer aussi sur le point
   qu'il s'était réservé : la forme finale de la contrainte `header-seal`
   (`CHDR-035`).
3. **Correction, lot A** — les neuf findings de test, sur sa propre branche. Les
   fixtures `Recipient::owner` de `cucumber.rs` et `g2_rotation.rs` ont migré ;
   le fichier n'a plus besoin d'être ouvert qu'une fois.
4. **Triage** — `CHDR-029` (P2) et `CHDR-030` à `CHDR-036` (P3) entrent en
   file. Aucun n'est assigné par cette revue.
