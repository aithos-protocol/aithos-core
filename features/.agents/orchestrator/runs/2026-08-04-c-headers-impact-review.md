> **`SUPERSEDED` — 2026-08-04.** Ce rapport est conservé, non corrigé et non
> effacé, parce qu'il est la seule preuve dure que le fil peut se tromper avec
> assurance, et que l'effacer supprimerait le moyen de mesurer si le correctif
> de méthode fonctionne.
>
> **Ce qui est faux.** §C.4 et §C.5 affirment qu'aucun chemin de migration
> n'existe et que la rétention de `spec/03-headers.md` §3.5 est inconditionnelle.
> Les deux sont contredits par le texte de la spécification elle-même.
>
> **Pourquoi.** Les dix axes de recherche `R1`–`R10` de ce rapport prennent tous
> `rust/**`, `vectors/` et `cucumber.rs` pour source ; **aucun ne prend `spec/`**.
> Le rôle a prouvé une absence dans le code par `git grep`, puis l'a énoncée du
> protocole. Le rapport porte 57 affirmations d'absence et une seule dit sur quoi
> elle a cherché. C'est la faute même que l'audit traque : une affirmation prouvée
> étroitement, énoncée largement. La responsabilité première est celle de
> l'orchestrateur, dont le brief faisait de `spec/` un index de citations et non
> une source à lire.
>
> **Remplacé par** le rapport de la relance aveugle, cut depuis `c547ccd` —
> antérieur à ce fichier — de sorte que le nouveau rôle ne peut pas le lire.
> La comparaison des deux est le test du correctif de méthode.

# Revue d'impact Gherkin globale — `c-headers`, cycle « autorité I3 »

## Identité du run

| Champ | Valeur |
|---|---|
| Date | 2026-08-04 |
| Type de run | revue d'impact inter-features |
| Rôle | G1 — `review-gherkin-impacts` (orchestrateur) |
| Unité de revue | `CHDR-I3-GLOBAL-IMPACTS` |
| Feature source | `features/c-headers.feature` |
| Révision auditée (baseline immuable) | `a2087f2` |
| Candidat accepté | `9dc5889` |
| Tête observée | `c547ccd` — `codex/fix-c-headers-i3-authority` |
| Plage observée | `a2087f2..c547ccd`, **trois mouvements distincts** : `5be3047` (spec `SI3-1..SI3-10`), le changement de format filaire du `kid` de la ligne owner, `9dc5889` (code) |
| Audit public source | `docs/audits/features/c-headers.md`, dont la **§6bis** (`CHDR-028`..`CHDR-036`) |
| Décision autorisante | `features/.agents/c-headers/decisions/2026-08-03-chdr-007-012-i3-authority.md` |
| Correction | `features/.agents/c-headers/corrector/runs/2026-08-04-correction-i3-authority.md` |
| Revue acceptée | `features/.agents/c-headers/auditor/runs/2026-08-04-review-i3-authority.md` — `CHDR-007` et `CHDR-012` `VERIFIED` |
| Proposition | `docs/PROPOSITION-SPEC-I3-AUTHORITY-2026-08-03.md` |
| Précédents de forme | `../runs/2026-07-29-a-identity-impact-review.md`, `../runs/2026-08-03-b-derivation-impact-review-02.md` |
| Arbre de travail | `/root/work/aithos-core`, dépôt complet avec `.git`, lecture seule |
| Résultat | **aucun `FULL_AUDIT`** ; **huit `TARGETED`**, **dix `NONE`** ; **deux conditions de blocage prononcées** (§D et §C.5), aucune classification laissée indécise |

Cette note n'est pas un audit sémantique en deux passes et n'en revendique
aucune. Elle part de l'audit accepté, des rapports de run, de la décision et du
diff ; la preuve comportementale du candidat reste celle de
`auditor/runs/2026-08-04-review-i3-authority.md`. **Aucune commande `cargo`,
aucun test, aucun build n'a été lancé par ce rôle** — la consigne de rôle
l'interdit et aucun résultat d'exécution n'est ici revendiqué comme observé par
moi. Les seuls faits d'exécution cités le sont **par attribution** à un
`evidence_id` du journal `runs/2026-08-04-r1/`. **Aucun fichier de feature,
d'audit public, de spec, de code, de vecteur, de `STATE.md` ni `QUEUE.yaml` n'a
été modifié.** Le seul fichier écrit est ce rapport.

## Conditions d'entrée — vérifiées, non re-débattues

1. `features/.agents/c-headers/STATE.md` porte la revue acceptée et nomme le
   relecteur d'impact global comme rôle suivant.
2. La revue indépendante conclut **`VERIFIED`** — pas seulement `IMPLEMENTED` —
   pour `CHDR-007` et `CHDR-012`, chacun contre son critère de clôture écrit.
3. Je ne juge pas la justesse de la correction. Elle est acceptée.
4. Les neuf findings de la §6bis sont `OPEN` et **non assignés** ; ils ne
   bloquent pas cette revue et je ne les audite pas.

## Périmètre réel du changement — revérifié, pas repris sur parole

### Mouvement 1 — le lot de spécification `5be3047`

Dix amendements, six fichiers, revérifiés par `git show 5be3047 -- spec/` :

| Fichier | Ce qui change |
|---|---|
| `spec/00-overview.md:35-40` | I3 réécrit en voix active : la ligne owner est **celle dont la clé destinataire est l'`owner_kex` du sujet** ; un vérificateur d'édition **DOIT** analyser tout header épinglé et rejeter l'édition ; « The routing label `to` never establishes the owner line and never satisfies I3 » |
| `spec/00-overview.md:85-93` | **l'obligation est rétroactive** et lie tout profil `aithos-core`, historiques compris ; motif écrit : une règle liée au seul profil récent serait contournée en publiant sous `draft.2`. Pas de `draft.3` |
| `spec/03-headers.md:20` | l'exemple filaire passe de `"kid": "owner-kex"` à `"kid": "z6LSOwnerKex…"` |
| `spec/03-headers.md:34-48` | `kid` nomme la clé ; unicité des `kid` dans une `key_version` ; définition de la ligne owner par le sceau ; **deux paliers** de vérification — keyless et porteur d'`owner_kex` |
| `spec/03-headers.md:52-58` | §3.2 Reading : l'owner résout la ligne dont le `kid` vaut son `owner_kex` ; « a successful unseal — never a label — is what proves the line was its own » |
| `spec/03-headers.md:107-113` | §3.4 : le révocateur re-scelle DK′ sur l'`owner_kex` **lu du document DID**, jamais sur la clé que portait la ligne owner précédente |
| `spec/05-delegation.md:89-92` | même obligation côté délégation : « a rotation that reproduces a wrong owner line propagates it, and I3 makes the whole edition invalid » |
| `spec/06-revocation.md:33-34` | l'algorithme `revoke` re-scelle la ligne owner sur `owner_kex` (§03.1) |
| `spec/09-cli-and-conformance.md:47-54, :100-102` | I3 obtient sa propre famille de vecteurs, quatre cas nommés, chacun déclarant son palier ; le **Core reader** DOIT rejeter une édition épinglant un header violant I3, **sans détenir aucune clé, sur tout profil de manifeste** |
| `spec/10-threat-model.md:19` | la ligne « Owner un-lockable-out » nomme désormais la clé destinataire et le vérificateur d'édition |

Portée : l'obligation ne dépend d'aucun profil et ne crée aucun construit signé.
**Elle lie donc rétroactivement toute édition du corpus et hors corpus.**

### Mouvement 2 — le format filaire

`Recipient::owner` (`rust/crates/aithos-core/src/header.rs:31-38`) pose désormais
`kid = owner_kid(&pubkey)` — `wire::x25519_pub_to_multibase` (`header.rs:18-20`)
— au lieu du littéral `"owner-kex"`.

Fait **établi par exécution** et attribué à `ev-15f8f483` : la ligne owner
construite et celle du vecteur C3 ne diffèrent **que par `kid`** ; `epk`, `n` et
`c` sont identiques caractère pour caractère. `kid` n'entre pas dans l'AAD de
ligne (`seal.rs`, purpose `header-line`, lié à `subject_did ‖ node ‖
key_version`). **Aucun chiffré n'est re-dérivé ; aucun vecteur épinglé à l'octet
n'est invalidé.** Contrôle indépendant de ma part :
`vectors/c1-header-seal.json` ne porte aucune clé `kid` (parsing JSON, liste de
clés : `vector, description, subject_did, node, key_version, dk_hex,
owner_kex_sk_hex, owner_pub_hex, grantee_sk_hex, grantee_pub_hex, owner_line,
grantee_line, wrap`). `git diff --stat 5be3047..9dc5889 -- vectors/` est vide.

Ce qui change en revanche : **les octets JCS de tout `header.json` produit**, donc
son `header_hash` (`rust/crates/aithos-bundle/src/state.rs:243`) et par
conséquent la racine d'état qui le replie. Aucun vecteur du dépôt n'épingle un
`header_hash` ni une racine d'état — vérifié par
`git grep -ln 'header_hash\|header_leaf\|state_root' -- vectors/`, qui ne rend
rien. C'est ce qui borne l'impact `h-merkle`.

### Mouvement 3 — la correction `9dc5889`

Quinze fichiers `rust/`, 878 insertions / 81 suppressions, plus deux fichiers de
test nouveaux. Cinq signatures publiques de `aithos-core::header` changent —
rupture d'API :

| Symbole | Ligne | Delta |
|---|---:|---|
| `Header::build` | `header.rs:128` | prend `owner_kex: &XPublicKey` |
| `Header::build_at` | `header.rs:154` | idem |
| `Header::rotate` | `header.rs:224` | idem |
| `Header::validate` | `header.rs:371` | prend `owner_kid: &str` ; compare `l.kid`, plus `l.to` |
| `Header::check_rotation` | `header.rs:334` | prend `owner_kid: &str` ; idem |

Quatre API nouvelles : `owner_kid` (`:18`), `open_owner` (`:285`),
`open_owner_latest` (`:296`), `validate_as_owner` (`:385`).

Deux vérificateurs d'édition gagnent une passe I3 keyless :
`bundle::verify_pinned_headers` (`bundle.rs:302-320`), appelée depuis
`Bundle::verify` (`bundle.rs:1759`) avec le document DID lu en `bundle.rs:1693` ;
et `publication::cold_verify` (`publication.rs:889-897`). Le `kid` attendu est
`doc.keys.kex` **tel quel** (`bundle.rs:311`) — byte-identique à `owner_kid()`
parce que `did.rs:91` et `header.rs:19` appellent le **même encodeur**,
`wire::x25519_pub_to_multibase` (`wire.rs:38`). Ce point est vérifié et il est
le pivot du §B.

Treize sites de lecture migrent vers `open_owner*` ; trois sites qui décidaient
quelle ligne remplacer sur `line.to == "owner"` comparent désormais le `kid`
(`revoke.rs`, `structure.rs`, `vault.rs`).

## Recherches effectuées

| # | Objet | Commande / motif |
|---|---|---|
| R1 | Périmètre exact des trois mouvements | `git show --stat` / `git show` de `5be3047` et `9dc5889`, par sous-arbre |
| R2 | Symboles changés — tous les appelants | `git grep` de `Header::build`, `build_at`, `rotate`, `validate`, `check_rotation`, `Recipient::` sur `rust/**` |
| R3 | Survivance du littéral `"owner-kex"` | `git grep -l 'owner-kex'` sur l'arbre suivi entier, puis tri code / vecteur / doc / archive |
| R4 | Steps partagés touchant header / ligne / rotation / wrap | extraction des attributs `#[given/when/then]` autour des **douze** sites modifiés de `cucumber.rs`, puis `grep -rl` de chaque phrase sur les **dix-neuf** `features/*.feature` |
| R5 | Surfaces d'édition | `git grep -n 'cold_verify\|\.verify()'` sur `cucumber.rs` (39 sites), remontée à la fonction englobante, puis à la phrase de step, puis à la feature |
| R6 | Formats — quels artefacts encodent un `kid` de ligne owner | parsing JSON de `c1-header-seal.json`, `g2-rotation.json`, `c3-owner-line.json` ; `git ls-files` sur `header|hdr/` ; `git grep` des motifs `header_hash`, `state_root` sur `vectors/` |
| R7 | Chemins de header écrits par la production | `git grep -n 'header.json\|hdr/'` sur `aithos-bundle/src` (13 sites) confronté au prédicat `is_header_file` (`bundle.rs:291-295`) |
| R8 | Sections de spec citées par les features | extraction des motifs `spec NN.N` / `§NN.N` / `NN.N` sur les 19 `.feature` |
| R9 | Surfaces `aithos-wasm`, `aithos-owner`, SDK | `Cargo.toml` de `aithos-wasm` (dépend de `aithos-core` seul) ; `git grep 'aithos_core::header\|Header::\|Recipient::'` sur `aithos-wasm/src`, `aithos-owner/src`, `aithos-cli` ; lecture de `aithos-bundle/src/sdk.rs` |
| R10 | Chaîne d'épinglage de `g2-rotation.json` | `git grep -ln 'g2-rotation'`, puis lecture de l'inventaire du correcteur et contrôle des consommateurs `cb2_*` |
| R11 | Couplage `keys.kex` ↔ `owner_kid` | `git grep -n 'x25519_pub_to_multibase'` sur `rust/**` (14 sites) ; lecture de `did.rs:91`, `did.rs:140`, `header.rs:19`, `grants.rs:172-181` |
| R12 | Chemin de transition d'époque | `git grep -n 'Transition\|epoch'` sur `aithos-core/src/did.rs`, `aithos-bundle/src`, `aithos-owner/src` |
| R13 | Verdicts des deux features `COMPLETE` | `grep -i 'I3\|owner line\|header\|03\.1\|kex'` sur `docs/audits/features/a-identity.md` et `…/b-derivation.md` |

Exhaustivité, énoncée franchement : R2, R3, R6, R8 et R11 sont des `git grep`
sur l'arbre suivi entier, donc exhaustifs à la casse et à l'orthographe près des
motifs. R4, R5 et R7 sont des balayages sur des ensembles clos (19 fichiers
`.feature`, 39 sites `verify`, 13 chemins de header). Ce qui n'est pas couvert
est énoncé au §7.

---

## A. Classification par feature — les dix-huit autres

Résumé : **0 `FULL_AUDIT`, 8 `TARGETED`, 10 `NONE`.**

| # | Feature | Classement | Motif tenant en une ligne |
|---:|---|---|---|
| 1 | `a-identity` | **`TARGETED`** | `keys.kex` devient le champ qui **définit** I3 ; la transition d'époque déplace ce champ sans chemin de migration des headers |
| 2 | `b-derivation` | `NONE` | dérivation pure, aucun header ; `b2-derivation.json` ne porte aucun `kid` |
| 3 | `d-bundle` | **`TARGETED`** | `d-bundle.feature:146` consomme la signature changée ; `Bundle::verify` gagne la passe I3 et `publish` n'en a aucune |
| 4 | `e-mandate-sections` | `NONE` | le grant de section produit un header dont seul le `kid` bouge ; aucun scénario n'assertit dessus |
| 5 | `e-mandates` | `NONE` | idem ; le plan mandat est intact |
| 6 | `f-gamma` | `NONE` | plan Gamma ; « rotate » y est un **domaine d'opération**, pas la rotation de header |
| 7 | `f-plus-constraints` | `NONE` | contraintes de mandat ; aucun contact header |
| 8 | `g-plus-obligations` | `NONE` | obligations et reçus ; aucun contact header |
| 9 | `g-revocation` | **`TARGETED`** | consomme `check_rotation(v, owner_kid)` ; §03.4/§05.5/§06.2 amendés visent exactement sa rotation |
| 10 | `g4-client-surfaces` | **`TARGETED`** | rupture de ligne de commande sur les deux seules surfaces CLI de §03, non exercées par un test |
| 11 | `h-merkle` | `NONE` | le `header_hash` change de valeur mais aucun vecteur ne l'épingle |
| 12 | `h2-gamma-roots` | `NONE` | racines Gamma ; aucun contact header |
| 13 | `i-concurrency` | `NONE` | fork/merge inchangés ; l'atomicité de `move` appartient à `n-structural-mutations` |
| 14 | `k-integration` | **`TARGETED`** | `cold_verify` gagne la passe **et** une précondition `did.json` nouvelle sur le chemin d'export que cette feature possède |
| 15 | `l-delegated-writes` | `NONE` | écrit via le chemin grant ; aucune assertion sur la ligne owner |
| 16 | `m-delegated-editions` | **`TARGETED`** | `spec/05-delegation.md:89-92` amendé : une édition déléguée devient invalide si une rotation reproduit une mauvaise ligne owner |
| 17 | `n-structural-mutations` | **`TARGETED`** | `move_folder` écrit l'index avant la garde I3 et n'est pas transactionnel ; `structure.rs:266` résout la clé depuis `to` |
| 18 | `o-connector-classes-vault` | **`TARGETED`** | `vault.rs:381` résout depuis `to` ; `vault.rs:334` ne valide pas ; `e/x/<connector>/header.json` est désormais vérifié à l'édition |

### A.1 `a-identity` — `TARGETED`

Deux couplages nouveaux, aucun n'existait avant `9dc5889`.

**(i) `keys.kex` devient le champ autorisant de I3.** `verify_pinned_headers`
prend le `kid` attendu directement de `doc.keys.kex` (`bundle.rs:311`), et tout
écrivain de header lit la même valeur (`grants.rs:172-176`,
`owner_kex_pub()`). La byte-identité tient **par construction** — `did.rs:91` et
`header.rs:19` appellent `wire::x25519_pub_to_multibase` (`wire.rs:38`) — et je
l'ai vérifiée, elle n'est pas supposée. Mais elle n'est **prouvée par aucun
test** : rien n'échouerait si l'un des deux encodeurs divergeait un jour.

**(ii) La transition d'époque.** `a-identity.feature:106-118` possède la règle
« Only the succession key can declare a new master key ». Une identité
successeur a un nouveau maître, donc un nouvel `owner_kex`, donc un nouveau
`keys.kex`. Tout header écrit sous l'époque précédente porte l'ancien
`owner_kid` ; après transition, `verify_pinned_headers` lit le document DID
**courant** et refuse chaque header ancien — donc **l'édition entière**. Sous le
code audité (`l.to == OWNER_LABEL`), la transition d'époque n'avait aucune
conséquence côté header. Elle en a une maintenant.

`EpochTransition` est purement `aithos-core` (`did.rs:166-280`) : `git grep`
sur `aithos-bundle/src` et `aithos-owner/src` ne rend **aucun** site
`transition` ni `epoch`. Le couplage est donc **latent** — non exercé, donc non
rouge — et c'est précisément pourquoi il doit être nommé ici plutôt que
découvert plus tard.

**Dû par un futur cycle `a-identity`** : (1) un scénario ou une vérification
établissant que `keys.kex` et `owner_kid()` sont le même encodage, en négatif ;
(2) l'énoncé de la conséquence header d'une transition d'époque — soit un
chemin de migration, soit une déclaration écrite qu'il n'y en a pas.

### A.2 `d-bundle` — `TARGETED`

Preuve de consommation directe : `features/d-bundle.feature:146` porte la ligne
d'Examples `| wrap | node-version-and-recipient header line | … |`, dispatchée
en `cucumber.rs:3100` vers `core_header_capability_scenario`
(`cucumber.rs:3041`), dont le corps appelle `Header::build` avec le nouvel
argument `&owner.owner_kex_pub()` (`cucumber.rs:3054`) puis
`session.append_header_recipient` (`session.rs:355-366`), qui appelle
`validate(&owner_kid)`. C'est la seule feature hors `c-headers` dont un scénario
traverse une signature changée par un chemin nommé dans son propre tableau.

Second motif, plus lourd : `Bundle::verify` (`bundle.rs:1759`) est le
vérificateur d'édition de `d-bundle`, et il gagne une raison de refus entièrement
nouvelle. `Bundle::publish` (`bundle.rs:1678`) n'en gagne aucune — l'émetteur
signe donc des éditions que son propre vérificateur refuse (`CHDR-034`).

**Dû** : (1) un scénario où une édition épinglant un header sans ligne owner est
**refusée** par `verify` ; (2) l'énoncé explicite, en scénario ou en règle, que
`publish` ne porte pas cette garde — ou qu'il doit la porter ; (3) la note du
suivi `bder-006-d-bundle` (`QUEUE.yaml:112`) reste due et se cumule.

### A.3 `g-revocation` — `TARGETED`

`features/g-revocation.feature:71-74` (« A rotation that smuggles in a new
recipient is rejected ») est câblé sur `smuggle_recipient`
(`cucumber.rs:15255-15295`), qui appelle `Header::rotate` avec `&owner_pub`
(`:15282`) puis `check_rotation(2, &owner_kid(&owner_pub))` (`:15292`). **Les
deux signatures changées passent par cette feature.**
`features/g-revocation.feature:67` et `:87` sont câblés sur `owner_rotates`
(`cucumber.rs:15241`), qui appelle `w.rotate(...)` → `revoke::rotate_folder`.

Les amendements `spec/03-headers.md:107-113`, `spec/05-delegation.md:89-92` et
`spec/06-revocation.md:33-34` portent **tous les trois** sur l'algorithme de
révocation-rotation, et tous les trois exigent la même chose : la ligne owner de
la nouvelle version est re-scellée sur l'`owner_kex` **lu du document DID**,
jamais sur la clé que portait la ligne owner précédente. Aucun scénario du
corpus n'exerce cette distinction.

`CHDR-029` place en outre `revoke.rs:188` (`rotate_folder`) et `revoke.rs:396`
(`move_folder`) parmi les quatre sites qui reconstruisent la clé du survivant
depuis `to` et non depuis `kid`.

**Dû** : (1) une rotation dont le header source porte une ligne owner scellée à
une clé **autre** que l'`owner_kex` du document DID, et le constat que la
nouvelle version re-scelle sur le DID et non sur l'ancienne ; (2) une rotation
sur un header portant une ligne `{ to: A, kid: B }`, et le constat de quelle clé
scelle la nouvelle version ; (3) `check_rotation` reste une **inclusion** là où
`spec/03-headers.md:93-96` demande une égalité (`CHDR-024`, non assigné) — à
conserver comme dette visible du cycle `g-revocation`.

### A.4 `n-structural-mutations` — `TARGETED`

`features/n-structural-mutations.feature:4` promet que « Their indexes, tag
views, rotations, wraps, Gamma and editions commit **atomically** », et `:56-60`
(« A move rotates at the changed cryptographic boundary ») demande que « required
rotation, survivor lines and destination up-link wrap **join the transaction** ».

`CHDR-031` établit que `Bundle::move_folder` (`revoke.rs:324`) n'est enveloppé
dans aucune `self.transaction`, écrit `e/circle/index.json` en `:422`, **puis**
appelle `Header::build_at` en `:431`, dont la première instruction est la garde
`check_owner_line` (`header.rs:164`) — garde que la correction vient de rendre
**plus stricte**, puisqu'elle compare désormais une clé et un `kid` au lieu d'un
label. La correction n'a pas créé le défaut d'ordonnancement ; elle a **élargi
l'ensemble des entrées qui le déclenchent**. C'est exactement l'énoncé
d'atomicité de cette feature qui devient réfutable.

`CHDR-029` place `structure.rs:266` (`structural_recipients`) parmi les quatre
sites qui résolvent la clé depuis `to`.

**Dû** : (1) un scénario de `move` sur un header sans ligne owner conforme, avec
constat de l'état du store après l'échec ; (2) le cas `to != kid` sur le chemin
structurel ; (3) le suivi `b-derivation` déjà inscrit
(`QUEUE.yaml:111`) se cumule.

### A.5 `o-connector-classes-vault` — `TARGETED`

Trois raisons distinctes, chacune sur une ligne de code nommée.

1. `vault.rs:381` (`rotate_vault_connector`) est le quatrième site `CHDR-029`.
   `features/o-connector-classes-vault.feature:219-231` fait de la rotation de
   connecteur un scénario d'atomicité et d'isolation (« only /x/mail recipients,
   versions and roots may change »).
2. `vault.rs:334` (`read_vault_config_owner`) n'appelle **pas** `validate`, à
   rebours de son homologue `log.rs:424` qui l'appelle
   (`CHDR-030`, `CHDR-036`).
3. `e/x/header.json` et `e/x/<connector>/header.json`
   (`bundle.rs:610`, `grants.rs:461`, `vault.rs:56`) satisfont le prédicat
   `is_header_file` (`bundle.rs:291-295`) : le plan vault entre dans le champ du
   nouveau vérificateur d'édition. `features/o-connector-classes-vault.feature:222`
   (« fresh-store keyless verification receives no credential ») s'appuie sur ce
   vérificateur.

**Dû** : (1) une rotation de connecteur sur un header portant `to != kid` ;
(2) un scénario où le header d'un connecteur viole I3 et où la vérification
keyless fraîche le refuse ; (3) l'énoncé du silence de
`read_vault_config_owner`.

### A.6 `g4-client-surfaces` — `TARGETED`

`aithos header-seal` exige désormais `--owner-kex-hex` et construit lui-même la
ligne owner (`header_seal.rs:19-20, :40-44, :70`) ; `aithos header-open` exige
`--owner-kid` et valide avant toute ouverture (`header_open.rs:17, :35`).
**Rupture de ligne de commande.** `CHDR-035` établit que
`rust/crates/aithos-cli/tests/cli_surface.rs` n'invoque **ni l'une ni l'autre** —
constat que j'ai reconfirmé en listant les fonctions de test de ce fichier.

C'est la seule feature dont le domaine déclaré (`@cli @wasm`, ligne 1) couvre la
surface rompue, et elle est **la prochaine du train** (`QUEUE.yaml`, `order`,
deuxième entrée). L'écart résiduel que la revue consigne en `CHDR-035` —
`header-seal` ferme la production d'un header *sans* ligne owner mais ne ferme
pas celle d'un header dont l'`owner_kex` n'est pas celui du sujet — est le point
que le propriétaire s'était explicitement réservé.

Note de portée : `aithos-wasm` ne touche aucun header — `Cargo.toml` ne dépend
que de `aithos-core`, et `git grep 'Header::\|Recipient::\|aithos_core::header'`
sur `aithos-wasm/src` et `aithos-owner/src` ne rend rien. La moitié WASM de
cette feature est donc `NONE` ; la moitié CLI est `TARGETED`.

**Dû** : (1) `cli_surface.rs` exerce `header-seal` et `header-open` ; (2) au
moins un cas négatif : un header produit avec un `--owner-kex-hex` étranger,
épinglé dans une édition, refusé par `Bundle::verify`.

### A.7 `k-integration` — `TARGETED`

`features/k-integration.feature:159-170` possède la ronde d'export vers un store
frais, câblée sur `core_cold_roundtrip_scenario` (`cucumber.rs:2775`) via les
steps `cucumber.rs:9429` et `:9435`, qui appellent
`publication::cold_verify` (`:2833`, `:2842`). `cold_verify` est le second
vérificateur d'édition et il gagne la passe I3 (`publication.rs:889-897`).

Conséquence **nouvelle et non exercée** : la passe lit `did.json` du store et
échoue si le document manque, dès lors que le manifeste épingle un header
(`publication.rs:889-896`). Sur le chemin `height == 1`, `did.json` n'était
auparavant **pas** requis par `cold_verify` — il ne l'était qu'à partir de
`height > 1` (`publication.rs:906-912`). C'est une **précondition nouvelle sur le
paquet exporté**, exactement dans la feature qui promet « Offline E2E means
export into a genuinely fresh local store ».

`features/k-integration.feature:178-189` énumère cinq défauts d'artefact public
qui doivent faire échouer la ronde froide. Aucun n'est un header mutilé, aucun
n'est un `did.json` absent.

**Dû** : (1) une ligne d'Examples supplémentaire — un header épinglé violant I3 —
sur le scénario `:178` ; (2) le cas `did.json` absent d'un export de hauteur 1
épinglant un header ; (3) l'énoncé de la précondition dans la règle `:157`.

### A.8 `m-delegated-editions` — `TARGETED`

`spec/05-delegation.md:89-92` est amendé et vise nommément le délégué : le
révocateur — qui peut être un ancêtre, pas le sujet — re-scelle la ligne owner
sur l'`owner_kex` du document DID, « never to the recipient key the previous
owner line happened to carry: a rotation that reproduces a wrong owner line
propagates it, **and I3 makes the whole edition invalid** ».

`features/m-delegated-editions.feature:81` demande que le changeset « explains
content, index, root, header, wrap, Gamma, vault **and rotation** consequences ».
La conséquence header d'une rotation déléguée vient de changer de nature : elle
peut désormais invalider l'édition entière, et un délégué peut l'infliger au
sujet. `m-delegated-editions` consomme par ailleurs `cold_verify` via
`core_self_edition_scenario` (`cucumber.rs:2865`, step `:9334`).

**Dû** : (1) un scénario où une rotation déléguée reproduit la ligne owner
antérieure au lieu de re-lire le document DID, et où l'édition est refusée ;
(2) le changeset `:81` énonce cette conséquence.

### A.9 Les dix `NONE` — motif par feature

- **`b-derivation`** — `NONE`. La dérivation pure ne produit aucun header :
  `spec/03-headers.md:15-16` dit qu'un nœud jamais individuellement accordé n'en
  a pas. `vectors/b2-derivation.json` ne porte aucun `kid` (contrôle du run
  d'impact du 2026-08-03, §1, reconfirmé). Aucun des onze sites `B2Vector::load`
  ne touche un header. Voir §B pour la question de l'invalidation.
- **`e-mandates`, `e-mandate-sections`** — `NONE`. Le chemin `grant_section` /
  `grant_folder` appelle `Header::build` (`grants.rs:297-306`, `:473-483`), donc
  le `kid` des headers qu'elles produisent change ; mais aucun scénario de ces
  deux features n'assertit sur un `kid`, une étiquette ni un décompte de lignes.
  La phrase « A section grant delivers exactly one header line »
  (`e-mandate-sections.feature:10`) est de la prose d'en-tête, pas une
  assertion, et elle était déjà imprécise avant ce cycle — `grants.rs:301` scelle
  `[owner, recipient]`, soit deux lignes. Ce n'est pas un impact de ce cycle.
- **`f-gamma`, `f-plus-constraints`, `g-plus-obligations`, `h2-gamma-roots`** —
  `NONE`. Aucune occurrence de `header` dans les quatre fichiers. Les douze
  occurrences de « rotat » dans `f-gamma.feature` (`:227`, `:490-507`, `:544-546`)
  désignent le **domaine d'opération** `rotate` du plan Gamma et son unicité
  d'occurrence, jamais la rotation de header. Aucun symbole changé sur leur
  chemin.
- **`h-merkle`** — `NONE`. `state.rs:243` replie `header.json` dans le hachage de
  son nœud, et les octets JCS du header ont changé ; mais le repli est calculé à
  l'exécution des deux côtés, et **aucun vecteur n'épingle un `header_hash` ni
  une racine d'état** (`git grep` sur `vectors/`, aucun résultat). Le gate
  workspace vert `ev-8bfeccca` le corrobore. `h-merkle.feature:8-10` (« The
  header is folded into its node's hash ») et `:55` restent vrais.
- **`i-concurrency`** — `NONE`. Le chemin fork/merge de `cold_verify`
  (`publication.rs:925-950`) est inchangé, la passe I3 est insérée **avant** lui
  (`:889-897`). Le défaut d'atomicité que `CHDR-031` nomme est celui de
  `move_folder`, dont le contrat appartient à `n-structural-mutations` ; je ne le
  compte pas deux fois.
- **`l-delegated-writes`** — `NONE`. Les grants d'écriture traversent
  `Header::build` par `grants.rs`, mais aucun scénario n'assertit sur la ligne
  owner ni sur un `kid` ; les vérifications qu'elle consomme sont celles de
  `d-bundle` et `k-integration`, déjà classées. Le cas du délégué **qui
  republie un header** est un scénario de `m-delegated-editions`, pas le sien.

---

## B. Le cas particulier des deux features `COMPLETE`

`PROCESS.md` § *Impact review* point 5 : ce rôle « does not modify or restart any
feature », et « The decision to restart an audit remains manual ». Un
`FULL_AUDIT` est une **condition de blocage**, jamais une réouverture
automatique. Je prononce ci-dessous ce qui est dû et je m'arrête là.

### B.1 `a-identity` — `status: COMPLETE`, aucun verdict invalidé

Recherche : `grep -i 'I3\|owner line\|header\|03\.1\|kex'` sur
`docs/audits/features/a-identity.md` ne rend **aucune ligne**. Aucun verdict de
`a-identity` ne repose sur I3, sur la ligne owner, sur `spec/03-headers.md`, ni
sur la sémantique de `keys.kex` au-delà de son encodage. Les findings ouverts
`AID-003` et `AID-004` portent sur la clé de succession et sa garde froide.

**Aucun verdict n'est invalidé ni par l'obligation rétroactive ni par le
changement de format.** Verdict explicite, pas un silence.

Ce que ce cycle crée en revanche, et qui n'existait pas : `keys.kex`, champ dont
`a-identity` possède la vérification, est devenu **le champ autorisant de I3**
pour tout le protocole (`bundle.rs:311`) ; et la règle
`a-identity.feature:106-118` — la transition d'époque — a désormais une
conséquence header sans chemin de migration (§A.1, §C.4). C'est un **élargissement
de la portée** de la feature, pas une invalidation de ses conclusions.
Classement `TARGETED`, **sans réouverture** : la dette échoit au prochain cycle
`a-identity` s'il est demandé, ou au propriétaire.

### B.2 `b-derivation` — `status: COMPLETE`, aucun verdict invalidé

Recherche identique sur `docs/audits/features/b-derivation.md` : deux
occurrences seulement, `:527` et `:547`, toutes deux des **mentions de gate**
(`--tags @c-headers 8/8`) dans le journal d'exécution, aucune un verdict.
`b-derivation` conclut sur la dérivation `node_key` et les vues de tag ; ses
neuf findings `VERIFIED` (`BDER-001`..`006`, `008`, `009`, `011`) et ses quatre
ouverts (`BDER-007`, `010`, `012`, `013`) sont sans contact avec I3.

Contrôle de format : `vectors/b2-derivation.json` ne porte aucun `kid` ; la
primitive `wrap` (`wrap_aad`, `wrap_seal`, `wrap_open`) est **inchangée** par
`9dc5889` — le diff de `header.rs` ne touche pas la structure `Wrap`. La règle
`spec/03-headers.md` §3.4 « step 2bis » est amendée sur la **ligne owner de la
nouvelle version**, pas sur les octets du wrap.

**Aucun verdict n'est invalidé.** `NONE`.

### B.3 Ce que je prononce, et ce que je ne prononce pas

**Je ne prononce aucun `FULL_AUDIT`.** Les deux features `COMPLETE` conservent
l'intégralité de leurs verdicts. `a-identity` reçoit un `TARGETED` qui, par
définition inscrite dans `QUEUE.yaml` (« TARGETED means a future cycle of that
feature owes specific scenarios; it never reopens a feature by itself »),
n'ouvre rien.

---

## C. L'obligation rétroactive, en propre

### C.1 Ce que l'obligation dit exactement

`spec/00-overview.md:85-93`, mot pour mot : « Editions published before
specification revision `2026-08-03-i3-authority` are therefore re-verified under
it; this is the one retroactive tightening of this series ». Et
`spec/09-cli-and-conformance.md:100-102` : le Core reader « MUST reject an
edition pinning a header that violates I3 (§03.1) — without holding any key, and
**on every `aithos-core` manifest profile** ».

Le motif est écrit et il est bon : une règle gatée sur le profil récent serait
contournée en publiant sous `draft.2`. Je ne le rouvre pas.

### C.2 Inventaire dans le dépôt — combien d'artefacts portent l'ancien format

`git grep -l 'owner-kex'` sur l'arbre suivi entier rend **seize fichiers**. Après
tri :

| Classe | Fichier | Ancien format d'un `kid` de ligne owner ? | Rejoué par un test ? |
|---|---|---|---|
| Vecteur | `vectors/g2-rotation.json:6`, `:12` | **oui** — le littéral `"owner-kex"` gelé dans `old_kids` et `expected_survivor_kids` | **oui** — `rust/crates/aithos-core/tests/g2_rotation.rs`, 4 tests |
| Générateur | `vectors/gen-g.py:103` | oui, même littéral, produit le champ ci-dessus | non (aucun `gen-*.py` ne tourne en CI) |
| Générateur | `vectors/gen-c.py:116` | **non** — chaîne de contexte de dérivation `"aithos-core/v1/owner-kex"` | — |
| Code | `rust/crates/aithos-core/src/derive.rs:12` | **non** — `CTX_OWNER_KEX`, même chaîne de contexte | — |
| Code | `rust/crates/aithos-cli/src/cmd/header_seal.rs:40` | **non** — message d'erreur de l'option `--owner-kex-hex` | — |
| Test | `rust/crates/aithos-core/tests/g2_rotation.rs:21` | oui, `const G2_OWNER_KID` — miroir délibéré du vecteur | oui |
| Spec | `spec/01-identity-and-keys.md:13` | **non** — chaîne de contexte de dérivation | — |
| Doc / archive | `docs/PROPOSITION-…`, `docs/audits/features/c-headers.md`, `docs/research/topology-…`, `docs/audits/split/spl8-amputation.patch` | prose, ou ligne de contexte d'un patch d'archive | non |
| Journaux d'agent | `features/.agents/…` (5 fichiers) | traces de run | non |

**Bilan chiffré. Un seul vecteur du dépôt gèle l'ancien format, et il est
rejoué : `vectors/g2-rotation.json`.** Zéro bundle de test, zéro fixture
sérialisée, zéro paquet figé. `git ls-files` ne rend aucun `header.json` ni
`hdr/*.json` suivi : **toutes** les fixtures de header du corpus sont
construites à l'exécution. `vectors/c1-header-seal.json` ne porte aucun `kid`
(parsing JSON), donc le vecteur canonique du sceau de ligne est indemne.

### C.3 Le cas `g2-rotation.json`, jugé sur pièces

Le vecteur n'est **pas** un header sérialisé : `old_kids` et
`expected_survivor_kids` sont des **listes de `kid`**, aux côtés de `zAGENT1`,
`zAGENT2`, `zINTRUS`, qui ne sont pas davantage des clés réelles. Le test
`g2_rotation.rs` passe `G2_OWNER_KID = "owner-kex"` à `check_rotation`, qui
compare ce que son appelant lui donne : le vecteur reste **vrai dans sa propre
fiction**, et aucun chemin ne le fait traverser `Bundle::verify`.

L'épisode est journalisé honnêtement par le correcteur : la révision en place a
été instruite, implémentée, puis annulée par l'orchestrateur quand
`ev-8eab8e17` a montré que le digest est épinglé **quatre niveaux** dans la
tranche CB2 — `cb2-bundle-structure-vault.json`, `cb2-bundle-concurrency-final.json`,
`cb2-core-bundle-red-ledger.json` et quatre entrées de `ownership.json`, plus
deux constantes `VECTOR_SHA256`. Ce coût, que personne n'avait chiffré, survit
au cycle.

**Mon jugement d'impact, et il n'est pas neutre** : le vecteur affiche une forme
filaire que le code ne produit plus. Ce n'est pas une fausseté aujourd'hui, mais
c'est un piège daté pour le prochain lecteur, et la garde posée — un commentaire
sur `G2_OWNER_KID` — est du texte, pas un mécanisme. Je propose un suivi
`QUEUE.yaml` dont le **sujet** est la cascade CB2, pas I3 (§E, suivi 6).

### C.4 Hors du dépôt — la question que le propriétaire lira en premier

**Il n'existe aucun chemin de migration, et l'obligation rend bien des éditions
valides hier invalides aujourd'hui.** Établi, non supposé :

1. Le vérificateur compare `l.kid` à `doc.keys.kex` (`header.rs:371-378`,
   `bundle.rs:311`). Un header écrit par un binaire antérieur porte
   `kid: "owner-kex"`, qui n'égale aucune multibase `z6LS…`. `validate` renvoie
   `MissingOwnerLine`, `verify_pinned_headers` propage, `Bundle::verify` et
   `cold_verify` refusent **l'édition entière** — pas la seule lecture du nœud.
2. Le refus est **keyless** : le porteur des données ne peut pas le contourner
   en présentant sa clé. `validate_as_owner` est plus strict, pas plus permissif.
3. Il n'existe **aucune fonction de reprise** : `git grep` de `migrate`,
   `upgrade`, `rewrite_header` sur `aithos-bundle/src` et `aithos-owner/src` ne
   rend rien. La seule voie est une **rotation de chaque header** par un
   détenteur de l'`owner_kex` — qui produit une nouvelle version de clé, donc une
   nouvelle édition, donc un nouveau chaînon dans l'histoire signée. Ce n'est pas
   une migration silencieuse : c'est une réécriture visible du bundle.
4. `spec/03-headers.md` §3.5 impose la **rétention** des anciennes versions de
   clé. Une rotation n'efface donc pas les versions anciennes, et
   `Header::validate` (`:371-378`) boucle sur **toutes** les `key_versions`. Une
   rotation qui ajoute une version conforme **ne suffit pas** : les versions
   anciennes, portant `kid: "owner-kex"`, feront toujours échouer `validate`.
   **La seule issue conforme est de réécrire chaque version de clé de chaque
   header, ce que la rétention de §3.5 interdit par ailleurs.** C'est la tension
   la plus dure de ce cycle et elle n'est écrite nulle part dans le lot de spec.
5. Le même mécanisme frappe la **transition d'époque** (§A.1, §B.1) : un
   changement légitime d'`owner_kex` invalide tous les headers antérieurs, avec
   la même impasse §3.5.

Le correcteur a nommé la limite (« a bundle produced by an older binary is not
readable by this one ») et l'a renvoyée à la revue d'impact ; la revue l'a
renvoyée à `CHDR-033`. Ni l'un ni l'autre n'a instruit le point 4. Je le fais
ici, et il change la nature de la question : ce n'est pas « faut-il un outil de
migration », c'est « **la rétention de §3.5 et l'obligation rétroactive de §0.2
sont-elles compatibles pour un porteur de données existantes** ».

Portée pratique aujourd'hui : `aithos-core` est en `0.1.0-alpha.1`
(`rust/Cargo.toml:12`), aucun header ancien n'existe dans l'arbre, et le
`CHANGELOG.md` `[Unreleased]` ne mentionne ni la rupture d'API ni la rupture de
format (`CHDR-033`). Le coût est donc **aujourd'hui nul et demain non borné**.

### C.5 Condition de blocage prononcée

**Je prononce une condition de blocage sur la compatibilité §0.2 / §3.5.** Elle
n'est ni un `FULL_AUDIT` ni une réouverture : c'est une question de
spécification que ce rôle n'a pas autorité à trancher, que la décision du
2026-08-03 n'a pas vue, et qui appartient au propriétaire du protocole. Elle est
formulée en une phrase : *une édition antérieure à la révision
`2026-08-03-i3-authority` peut-elle être rendue conforme sans violer la
rétention des versions de clé de §3.5, et sinon, l'obligation rétroactive
admet-elle une exception pour les versions de clé retenues ?*

---

## D. Les neuf findings nouveaux — débordements, sans audit

Je ne les audite pas. Je dis seulement lesquels sortent de `c-headers` et vers
quelle feature.

| Id | P | Déborde de `c-headers` ? | Vers |
|---|---|---|---|
| `CHDR-028` | P2 | **voir ci-dessous — arrêt** | — |
| `CHDR-029` | P2 | **oui**, quatre sites de production | `g-revocation` (`revoke.rs:188`), `n-structural-mutations` (`revoke.rs:396`, `structure.rs:266`), `o-connector-classes-vault` (`vault.rs:381`) |
| `CHDR-030` | P3 | **oui**, quatre surfaces détentrices d'`owner_kex` | `d-bundle` (`bundle.rs:667`, `:674`), `o-connector-classes-vault` (`log.rs:427`, `vault.rs:334`), `l-delegated-writes` / `m-delegated-editions` (`session.rs:363`) |
| `CHDR-031` | P3 | **oui**, effet partiel non transactionnel | `n-structural-mutations` (contrat d'atomicité `:4`, `:56-60`) |
| `CHDR-032` | P3 | **partiellement** — l'unicité des `kid` est `c-headers` ; la surface d'émission est CLI | `g4-client-surfaces` (`header_seal.rs:53-57`) |
| `CHDR-033` | P3 | **oui**, mais hors feature | cross-cutting : `rust/Cargo.toml:12`, `CHANGELOG.md` — suivi `QUEUE.yaml`, pas une feature |
| `CHDR-034` | P3 | **oui**, l'émetteur d'édition | `d-bundle` (`bundle.rs:1678`, `publish`) |
| `CHDR-035` | P3 | **oui**, entièrement | `g4-client-surfaces` (les deux commandes CLI de §03) |
| `CHDR-036` | P3 | **oui**, quatorze sites de lecture sans `validate` | `e-mandates` / `e-mandate-sections` (`grants.rs:834`, `:1044`, `:1204`), `n-structural-mutations` (`structure.rs:192`, `:279`, `:752`), `g-revocation` (`revoke.rs:155`, `:303`, `:383`, `:526`), `o-connector-classes-vault` (`vault.rs:334`, `log.rs:399`) |

Note de cohérence : les débordements `CHDR-036` vers `e-mandates` et
`e-mandate-sections` ne changent **pas** leur classement `NONE`. `CHDR-036` est
un écart entre la spec amendée et le code, explicitement écarté par la décision
du propriétaire (« la validation sur les chemins de lecture » est la troisième
lecture, non retenue) ; il ne crée aucune dette de scénario pour ces deux
features tant que la spec n'est pas retranchée ou le code étendu. Il est listé
pour que la prochaine décision le voie, comme l'auditeur l'écrit.

### D.1 `CHDR-028` — arrêt et signalement

Titre neutre, et rien de plus :

> **couverture inégale de I3 entre les surfaces de vérification d'édition de
> `aithos-bundle`.**

`CHDR-028` est `OPEN`, P2, `disclosure: embargo`. **J'arrête l'analyse ici et je
signale.** Motif : router ce finding vers une feature exige de nommer la surface
concernée ; nommer la surface, c'est décrire le mécanisme. Ma recherche §R9 —
l'inventaire des surfaces de vérification de `aithos-bundle`, y compris
`sdk.rs` — m'a conduit au bord de cette description. Je n'écris pas ce que j'ai
vu, je ne le consigne dans aucun fichier, et je ne propose **aucun** suivi
`QUEUE.yaml` le concernant.

C'est une **condition de blocage** et elle appartient à l'orchestrateur, comme
l'écrit `auditor/runs/2026-08-04-review-i3-authority.md` §8. Le classement
d'impact de `CHDR-028` reste **indéterminé par embargo** — ce n'est pas une
classification manquante, c'est une classification retenue.

---

## E. Suivis proposés pour `QUEUE.yaml` — proposition, pas modification

Je ne modifie pas `features/.agents/orchestrator/QUEUE.yaml`. Contenu proposé
pour sa section `follow_ups`, à ajouter aux deux entrées existantes :

```yaml
follow_ups:
  # — existant, inchangé —
  b-derivation-round-2-targeted: [a-identity, c-headers, d-bundle, e-mandates, n-structural-mutations]
  bder-006-d-bundle: tag-view and wrap scenarios owed by the d-bundle cycle

  # — proposé par l'impact review c-headers du 2026-08-04 —
  chdr-i3-targeted: [a-identity, d-bundle, g-revocation, g4-client-surfaces, k-integration, m-delegated-editions, n-structural-mutations, o-connector-classes-vault]
  chdr-i3-g4-cli: header-seal and header-open are unexercised by cli_surface.rs; the g4 cycle owes both, with one negative case pinning a foreign owner_kex in an edition (CHDR-035, CHDR-032)
  chdr-i3-d-bundle: an edition pinning an I3-violating header must be refused by verify, and publish carries no such guard (CHDR-034, CHDR-030)
  chdr-i3-g-revocation: a rotation must re-seal the owner line to the DID owner_kex, not to the key the previous owner line carried (spec 03.4, 05.5, 06.2); plus the to!=kid survivor case (CHDR-029)
  chdr-i3-n-structural: move_folder writes the index before the I3 guard and is not transactional (CHDR-031); structural_recipients resolves from `to` (CHDR-029)
  chdr-i3-o-vault: connector rotation resolves from `to` (CHDR-029); read_vault_config_owner never validates (CHDR-030); e/x headers are now edition-verified
  chdr-i3-k-integration: cold_verify gained the I3 pass and a new did.json precondition at height 1; the cold round-trip owes a mutilated-header defect row
  chdr-i3-m-delegated: a delegated rotation reproducing a stale owner line invalidates the whole edition (spec 05-delegation.md:89-92)
  chdr-i3-a-identity: keys.kex is now the field that defines I3; an identity-epoch transition has an unstated header consequence and no migration path — TARGETED, not a reopening of a COMPLETE feature
  chdr-i3-cb2-pinning: revising one promoted vector costs four levels of pinning across the CB2 slice; g2-rotation.json still freezes the pre-variant-A wire form. Subject of any future lot is the CB2 cascade, never an I3 side effect
  chdr-i3-versioning: five public aithos-core::header signatures broke and the at-rest header format broke; rust/Cargo.toml still reads 0.1.0-alpha.1 and CHANGELOG [Unreleased] names neither (CHDR-033)
  chdr-i3-retention-vs-retroactivity: BLOCKING — spec 00-overview.md:85-93 re-verifies pre-revision editions under I3, while spec 03-headers.md 3.5 retains old key versions that can never be made conformant. Owner ruling required
  chdr-028: BLOCKING, embargo — routing requires naming the surface, which describes the mechanism. Held by the orchestrator; no target recorded here
```

Aucun changement proposé à `order`, `policy`, `budget`, `models` ni `yardsticks`.

---

## 7. Limites de cette conclusion

- **Je n'ai exécuté aucune commande de build ou de test.** Aucun résultat
  d'exécution n'est revendiqué comme observé par moi ; ceux que je cite sont
  attribués nommément à un `evidence_id` du journal `runs/2026-08-04-r1/` et je
  ne peux pas attester qu'ils ont tourné.
- Les gates verts (`ev-8bfeccca`, `ev-03c0fdfc` : 18 features / 114 rules /
  836 scenarios / 3577 steps) établissent l'**absence de régression** sur les
  scénarios existants. Ils n'établissent aucune **couverture** des obligations
  nouvelles : les huit `TARGETED` portent sur ce que ces gates ne peuvent pas
  prouver.
- R4 est exhaustif en dépistage (les 19 `.feature`, les 12 sites modifiés de
  `cucumber.rs`) et sélectif en traçage : j'ai remonté les phrases de step des
  sites modifiés, pas l'intégralité des 39 sites `verify`.
- La conséquence de la transition d'époque (§A.1, §C.4 point 5) est établie par
  **lecture de code**, pas par exécution : elle est latente, aucun chemin
  `aithos-bundle` ne l'exerce. Si vous voulez qu'elle soit prouvée avant d'être
  inscrite, demandez-moi le test et je vous le décrirai — je ne le lancerai pas.
- Le point §C.4-4 — l'incompatibilité entre la rétention de §3.5 et l'obligation
  rétroactive — est un **raisonnement de ma part** sur `header.rs:371-378` et
  `spec/03-headers.md` §3.5. Il n'a été ni exécuté ni contre-argumenté par un
  autre rôle. Il est présenté comme une question, pas comme un verdict.
- `CHDR-028` n'est pas classé, par embargo. Ce n'est pas un oubli.
- Je n'ai sous-traité aucune partie de cette analyse.

## 8. Prochaine action et rôle attendu

1. Le propriétaire lit §C, tranche la question §C.5 (rétention vs rétroactivité)
   et lève ou maintient l'embargo `CHDR-028`.
2. L'orchestrateur inscrit, s'il les accepte, les suivis du §E dans `QUEUE.yaml`.
3. Le train reprend son `order` : `g4-client-surfaces` est la prochaine feature
   et elle est `TARGETED` par cette revue — sa dette est nommée en §A.6.

Aucun rôle n'est attendu sur `c-headers`. Aucune feature n'est rouverte par ce
rapport.
