# Audit d'implémentation — `c-headers.feature`

## 1. Métadonnées

| Champ | Valeur |
|---|---|
| Feature auditée | `features/c-headers.feature` (`@c-headers`) |
| Ronde | 1 — audit initial, mode orchestré |
| Date | 2026-08-03 |
| Révision observée | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` (`a2087f2`) |
| Base `main` enregistrée | `a2087f2392389fb17e0bc0ba9e20a164d53766d8` |
| Branche | `codex/audit-c-headers-r2` |
| Run orchestré | `2026-08-03-r1` (`features/.agents/orchestrator/runs/2026-08-03-r1/`) |
| État du worktree | propre pour tout le périmètre audité ; `features/.agents/c-headers/STATE.md` modifié par l'orchestrateur (gel de révision) et `features/.agents/orchestrator/runs/2026-08-03-r1/` non suivi au moment de l'audit |
| Périmètre | la vérité sémantique des huit scénarios existants ; quatre blocs `Rule` |
| Préfixe de findings | `CHDR-*` (`docs/audits/features/README.md:20`) |
| Domaine | `features/.agents/c-headers/DOMAIN.md` |
| Rapport de run | `features/.agents/c-headers/auditor/runs/2026-08-03-audit-initial.md` |
| Étalon Pass B | branche `origin/codex/audit-c-headers` (`af32734`), audit manuel de juillet 2026 |

### Avertissement — collision d'identifiants avec l'étalon de juillet

La branche publique `origin/codex/audit-c-headers` porte un
`docs/audits/features/c-headers.md` daté du 2026-07-30 qui attribue déjà les
identifiants `CHDR-001` … `CHDR-016` à **d'autres énoncés** que ceux de la
présente note. Les deux documents sont publics et revendiquent la même famille
d'identifiants stables réservée par `docs/audits/features/README.md:20`.

Cette note n'a pas autorité pour renuméroter l'un ou l'autre jeu. Tant que la
collision n'est pas tranchée par le propriétaire humain, **tout renvoi à un
`CHDR-*` doit nommer sa source** : « `CHDR-nnn` (ronde 1, run `2026-08-03-r1`) »
ou « `CHDR-nnn` (étalon de juillet, `af32734`) ». La §8 donne la table de
correspondance complète entre les deux jeux. La collision n'est pas une
condition de blocage au sens de `PROCESS.md` § *Blocking conditions* — cette
liste est close — mais elle est signalée au propriétaire par la même voie que
la barrière de divulgation.

## 2. Provenance de la méthode

Mode orchestré. L'isolation du Pass A est **matérielle** : chaque unité de revue
a tourné contre un extrait `git archive` de `a2087f2` **sans répertoire `.git`**
(`ledger.jsonl`, entrées `role: extract`, `sha256:
589fcc39c257f05a7a639845c79c5d7f9886e585841a3c2f459f8503b02bba0c`). Aucun agent
de Pass A n'a exécuté de gate : l'orchestrateur seul exécute les gates, écrit
les transcripts et enregistre un `evidence_id`.

| Unité | `Rule` | Scénarios | Contamination Pass A |
|---|---|---|---|
| RU-1 | A line seals the node key to exactly one recipient | 1 à 4 | aucune |
| RU-2 | The owner line is mandatory (I3) | 5 | aucune |
| RU-3 | Grant is one appended line, touching nobody | 6 | aucune |
| RU-4 | Rotation cuts the revoked and re-links the parent | 7 et 8 | aucune |

Les quatre unités ont été gelées dans
`features/.agents/orchestrator/runs/2026-08-03-r1/pass-a/frozen.json` avant
l'ouverture du Pass B. Un panel de réfutation adverse a ensuite instruit les
**seize** findings P1/P2 gelés, à trois réfuteurs indépendants chacun, chaque
réfuteur ne recevant que l'énoncé du finding (`pass-a/refutation.json`,
`ledger.jsonl`, entrées `role: refutation`). Le Pass B, la passe d'état partagé
et la réconciliation ont été conduits en dernier, par l'auditeur intégrateur,
sur le dépôt complet.

**Divulgation de contamination.** L'auditeur intégrateur lit l'histoire, l'étalon
de juillet et les verdicts gelés : c'est la définition du Pass B. Aucune de ces
entrées n'a été visible d'une unité de Pass A. La ligne `counts` de
`frozen.json` est **erronée** (elle annonce P2=14 / P3=9) ; le décompte réel du
gel est P1=1, P2=15, P3=8, total 24. L'erreur est de comptage, pas de contenu :
la liste `findings` du même fichier est correcte et fait foi.

## 3. Verdict

La cryptographie de header est fidèle à `spec/03-headers.md`. **Aucun finding de
cette note ne demande une correction de `aithos-core`.** Ce qui est faible, ce
n'est pas le produit : c'est la preuve.

Trois constats structurent la ronde.

1. **Six des huit scénarios énoncent un fait structurel et prouvent une
   conséquence comportementale.** « le révoqué *n'a pas de ligne* » est prouvé
   comme « le révoqué n'ouvre pas » ; « *toute autre* ligne intacte » est prouvé
   sur un header qui n'a qu'une autre ligne ; « liée à son nœud *et à sa
   version* » ne fait varier que le nœud ; « le wrap *restaure la dérivation* »
   n'exécute aucune dérivation.
2. **Le scénario 8 ne prouve pas son énoncé.** Il scelle une constante sous une
   constante et la rouvre deux pas plus loin sous la même constante, sans header,
   sans rotation et sans dérivation. Le Pass A l'avait classé `PROXY` ; la
   réconciliation le requalifie en `SEMANTIC_FALSE_POSITIVE` (§5, §6
   `CHDR-021`).
3. **La liaison de version du sceau de ligne n'a aucun défenseur
   comportemental dans tout le dépôt.** Elle n'est tenue que par des épinglages
   d'octets contre des vecteurs, dont l'un n'a pas de générateur dans le dépôt
   (§6 `CHDR-025`). C'est le seul finding nouveau de sévérité P2 de la ronde, et
   il vient de la passe d'état partagé.

Deux findings appellent une décision humaine avant toute correction :
`CHDR-007` (P1) et `CHDR-012` (P2). Les deux ont été retenus par la barrière de
divulgation pendant le cycle, puis publiés en entier sur décision du
propriétaire le 2026-08-03 (§6, préambule ; trace complète en §15). Ils restent
`DECISION_REQUIRED` et ne sont assignés à aucun correcteur.

### Compteurs exacts

Cités par `evidence_id`, jamais recopiés d'un document.

```
ev-50caa5d6 — 1 feature / 4 rules / 8 scenarios (8 passed) / 28 steps (28 passed)
```

## 4. Preuves reproduites

Le rôle auditeur n'exécute aucun gate en mode orchestré
(`PROCESS.md` § *Orchestrated gate execution*, amendement AM). La propriété du
gate ne bouge pas : seule son exécution bouge.

| `evidence_id` | Commande | Rev | Exit | Compteurs |
|---|---|---|---|---|
| `ev-50caa5d6` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @c-headers` | `a2087f2` | 0 | 1 feature / 4 rules / 8 scénarios (8 passés) / 28 steps (28 passés) |
| `ev-d6840262` | `cargo test --manifest-path rust/Cargo.toml -p aithos-bundle --test cucumber -- --tags @a-identity` | `a2087f2` | 0 | 1 feature / 8 rules / 30 scénarios (30 passés) / 93 steps (93 passés) — gate de préchauffage, hors périmètre |

Transcripts : `features/.agents/orchestrator/runs/2026-08-03-r1/evidence/`.

Les compteurs de `ev-50caa5d6` correspondent exactement au fichier de contrat
(4 `Rule`, 8 `Scenario`, 28 pas) : c'est la preuve de sélection et d'exécution.

**Le code de sortie est ici probant, et il ne l'était pas en juillet.**
`cucumber.rs:19736-19745` appelle désormais `fail_on_skipped()` puis
`filter_run_and_exit`. C'est le correctif `BDER-011`, accepté le 2026-07-30. Sur
la branche étalon de juillet, `main()` appelait `filter_run`, qui rend son writer
et ne quitte jamais : sous `harness = false` le binaire sortait `0` avec des
scénarios en échec. **Aucun chiffre de gate provenant de la branche étalon n'est
cité dans cette note, et aucun ne peut l'être.**

Aucune exécution autre que les deux ci-dessus n'est revendiquée. Toute
affirmation de comportement dans cette note repose sur la lecture du code
courant à `a2087f2`, jamais sur une exécution non journalisée.

## 5. Matrice des scénarios

| # | Scénario | Pass A | Réconcilié | Chemin de production | Ce que l'assertion compare réellement |
|---|---|---|---|---|---|
| 1 | Owner and grantee each open their line | `PROVEN` | `PROVEN` | `Header::build` → `build_at` → `build_lines` → `seal_line` ; `Header::open` → `open_line` ×2 | deux clés recouvrées indépendamment, chacune `assert_eq!` contre `DK` ; le filtre `kid` prouve que chaque destinataire a ouvert sa propre ligne |
| 2 | A non-recipient opens nothing | `PROVEN` | `PROVEN` | `Header::build` ; `Header::open` ×2 sous `xsk(0x99)` | `!opened.is_empty()` puis `all(is_err)` ; le nombre de tentatives n'est jamais lié au nombre de lignes |
| 3 | A corrupted line fails closed | `PARTIAL` | `PARTIAL` | `Header::build` ; bascule d'un caractère hex sur `lines[0].c` ; `Header::open` | `opened.last().is_err()` — sans contrôle positif interne ; la scène n'établit jamais que la ligne s'ouvrait avant la corruption |
| 4 | A line is bound to its node and version | `PARTIAL` | `PARTIAL` | `Header::build` ×2 ; greffe de ligne ; `Header::open` sous l'AAD d'un autre nœud | `opened.last().is_err()` ; seule la composante `node` de `line_aad` varie, `key_version` reste 1 des deux côtés |
| 5 | A header without an owner line is invalid | `PARTIAL` | `PARTIAL` | `Header::build` → `build_at` → `check_owner_line` → `Err(MissingOwnerLine)` | le `When` panique sur `Ok` ; le `Then` assère que l'erreur *stringifiée* contient `"I3"` ; un seul des quatre portails I3 du code est exercé côté fail-closed |
| 6 | Granting a new reader leaves every other line untouched | `PARTIAL` | `PARTIAL` | `Header::append_line` → `seal_line` ; `Header::open` | clé recouvrée `== DK` ; identité d'octets `PartialEq` de la ligne owner contre un instantané pré-append — sur un header dont l'ensemble « toute autre ligne » a le cardinal 1 |
| 7 | The revoked gets no line in the new version | `PARTIAL` | `PARTIAL` | `Header::build` → `Header::rotate` → `build_lines` ; `Header::open` ×3 | survivant et owner recouvrent `DK2` en v2 (fort) ; le rejet du révoqué est produit par le filtre `kid`, jamais par le sceau ; `key_versions["2"].lines` n'est lu par aucune assertion |
| 8 | An up-link wrap restores derivation for the parent holder | `PROXY` | **`SEMANTIC_FALSE_POSITIVE`** | `Wrap::seal` → `wrap_seal` → `derive_key(CTX_WRAP_KEY, …)` ; `Wrap::open` | un aller-retour AEAD symétrique sous la constante même qui a servi à sceller, dans le même scénario, sans header, sans rotation et sans dérivation |

Totaux réconciliés : **2 `PROVEN`, 5 `PARTIAL`, 1 `SEMANTIC_FALSE_POSITIVE`**.

### Pourquoi le scénario 8 n'est pas `PROXY`

`PROXY` désigne un scénario qui « consomme un verdict partagé sans exécuter son
propre cas ». Le scénario 8 exécute bien son propre cas : `post_uplink_wrap`
(`cucumber.rs:8164-8174`) construit un `Wrap` réel et `parent_recovers_via_wrap`
(`:12396-12404`) l'ouvre réellement. Ce qu'il ne fait pas, c'est prouver ce que
sa phrase énonce — la définition exacte de `SEMANTIC_FALSE_POSITIVE`. Les trois
composantes de la phrase sont absentes du code exécuté :

- « a parent holder » — `PARENT_KEY` (`cucumber.rs:265`) n'est la sortie d'aucun
  `node_key`, n'est ouverte d'aucune ligne de header, et n'est la clé d'aucun
  nœud du scénario ;
- « the new node key » — `DK2` (`:264`) n'est produite par aucune rotation ici ;
  `w.header` reste `None` pendant tout le scénario ;
- « restores derivation » — aucun `node_key`, aucun `folder_label`, aucun lien
  parent→enfant n'est calculé.

Ce qui est établi est exactement `wrap_open(wrap_seal(k, dk)) == dk`.
`Wrap::open` (`header.rs:351-353`) recalcule son AAD depuis ses **propres**
champs `self.node` et `self.key_version` : l'assertion ne peut donc pas détecter
un wrap posté sous le mauvais nœud ni sous la mauvaise version. Et `via`
(`header.rs:344`) n'entre pas dans `wrap_aad` (`seal.rs:41-43`) : il est stocké,
lu par personne.

## 6. Findings

Statut du panel noté `n/3 réfutations`. Un finding survit sur une majorité de
non-réfutations. Un finding réfuté par une majorité **revient à l'auditeur comme
question ouverte** (`PROCESS.md` § *Adversarial refutation*) : la ligne
« réconciliation » de chaque bloc dit ce que le Pass B en a fait, sur preuve de
code courant.

### Barrière de divulgation — levée le 2026-08-03

`aithos-core` est public et cette branche y sera poussée. Le Pass A avait marqué
quatre findings `disclosure: embargo` : `CHDR-003`, `CHDR-007`, `CHDR-008` et
`CHDR-012`. **Aucun ne l'est plus.**

- `CHDR-003` et `CHDR-008` sont **retirés** par la réconciliation (§7) ; leur
  embargo tombe avec eux.
- `CHDR-007` et `CHDR-012` sont **publiés en entier sur décision du propriétaire
  humain**, enregistrée le 2026-08-03 :

> « Publier les deux en entier. `CHDR-007` est déjà public en substance sur
> `codex/audit-c-headers` ; `CHDR-012` est publié malgré l'absence de correctif,
> au motif que le correcteur doit pouvoir citer ce qu'il répare. »
>
> — Mathieu Colla, propriétaire du protocole, 2026-08-03. Run de reprise
> `2026-08-03-r2`.

La condition de blocage 9 est donc **résolue**. La barrière a réellement joué
pendant ce cycle et la trace en est conservée en §15 : ce n'est pas une
formalité rétroactive.

**Ce que la décision ne tranche pas.** La levée de l'embargo est une décision de
publication, non de sémantique. `CHDR-007` et `CHDR-012` restent tous deux
`DECISION_REQUIRED` : la question normative qu'ils posent — un invariant que la
spécification énonce à la voix passive lie-t-il une surface vérifiante, ou
décrit-il seulement une propriété d'objet ? — n'est pas tranchée et ne doit pas
l'être par un correcteur. **Ces deux findings ne sont assignés à aucun
correcteur** (§11 lot 0, §12, §15).

---

### `CHDR-007` — `DECISION_REQUIRED`, P1 — 1/3 réfutations (survit)

**La moitié « édition » de I3 n'est imposée par aucun vérificateur d'édition.**
**Scénario 5 / RU-2 — finding de surface publique.**

`spec/00-overview.md:33-34` et `spec/03-headers.md:36-37` énoncent I3 en **deux**
propositions :

> **I3 — Owner line.** Every header MUST contain a line for the owner. A header
> without one is invalid, **and so is the edition carrying it.**
>
> **I3:** every `key_versions[*].lines` MUST include the owner line. **An edition
> whose any header violates this is invalid.**

La première moitié est imposée en quatre points de `aithos-core`
(`check_owner_line` sur `build`/`build_at` à `header.rs:133`, sur `rotate` à
`:201`, la branche owner de `check_rotation` à `:298-303`, et `validate` à
`:308-315`). **La seconde ne l'est nulle part.**

`Bundle::verify` (`bundle.rs:1654-1769`), le vérificateur d'édition hors ligne,
contrôle le document DID (`:1656`), la chaîne et les signatures de manifestes,
la hauteur et `prev_hash`, les fusions et résolutions de fork, les digests
SHA-256 des fichiers épinglés, l'absence de fichier non épinglé, les liens gamma
et `gamma_head`, et les racines Merkle d'état et gamma. **Il n'appelle jamais
`Header::validate`** ; recherche exhaustive sur son corps entier
(`bundle.rs:1654-1769`) : aucune occurrence de `Header` ni de `validate`.

Le seul contact de la vérification avec les headers est indirect :
`header_hash_at` (`state.rs:57-62`, « `BLAKE3(JCS(header.json))` if the node was
ever granted, else zeros ») et `vault_build` (`state.rs:240-248`) les
désérialisent en `serde_json::Value` **opaque** pour en calculer le digest JCS.
Un header dépourvu de ligne owner y produit un hash parfaitement valide, qui est
plié dans la racine Merkle d'état, épinglé au manifeste, et signé.

**Portée élargie par un réfuteur, vérifiée :** `publication::cold_verify`
(`publication.rs:836-939`) est un **second** vérificateur d'édition, tout aussi
muet sur I3.

**Conséquence rattachée par le même réfuteur.**
`spec/10-threat-model.md:19` inscrit « Owner un-lockable-out » à la table des
menaces et n'y cite qu'une seule contre-mesure : « owner line mandatory in every
header (I3) ». Producteur possible identifié : un délégué signant une édition
ordinaire — branche `m.version == CORE_DRAFT2_VERSION` de `verify`,
`bundle.rs:1664` — qui publie une rotation dont la nouvelle `key_version` omet la
ligne owner et ré-encrypte sous une DK' aléatoire. Un header sans ligne owner ne
peut pas être *créé* par les constructeurs de `aithos-core`, mais un header
arrivant par une autre route — `header.json` édité à la main, bundle importé,
écrivain futur, aller-retour `serde` — serait haché dans l'arbre d'état, épinglé,
signé dans un manifeste, et passerait `verify` sans opposition.

### Les deux lectures concurrentes — exposées, non arbitrées

| | Lecture A — I3 est un invariant d'édition | Lecture B — I3 est un invariant de construction |
|---|---|---|
| Fondement | `spec/00-overview.md:33-34` et `spec/03-headers.md:36-37` disent « and so is the edition carrying it » / « An edition whose any header violates this is invalid » : la phrase vise l'édition, donc le vérificateur d'édition | la spécification énonce I3 à la **voix passive** et ne l'impose explicitement à aucun vérificateur ; aucun vecteur de `spec/09-cli-and-conformance.md` §9.2 ne gate le cas |
| Conséquence | `Bundle::verify` et `publication::cold_verify` doivent valider chaque header de l'édition | l'architecture actuelle — fail-closed à l'écriture (`header.rs:133`, `:201`) plus validation au parse (`header.rs:308-315`, appelée en `bundle.rs:630`, `:637`, `log.rs:425`, `session.rs:363`, `aithos-cli/src/cmd/header_open.rs:28`) — est **conforme** |
| Coût | parser chaque header à chaque `verify` | la phrase de spec doit être resserrée pour dire ce que le code fait |
| Porté par | l'auditeur et deux réfuteurs sur trois | le réfuteur dissident |

Une troisième lecture est ouverte et n'a été portée par personne : déplacer la
validation sur les seuls chemins de lecture. **Aucun correcteur ne peut choisir
implicitement.** `DECISION_REQUIRED`, propriétaire attendu : le propriétaire du
protocole.

**Réconciliation.** Maintenu à P1. Le Pass B confirme la lecture du Pass A sur le
code courant et ne l'élargit pas ; il y absorbe `CHDR-008` (§7), dont l'énoncé
— la couverture inégale de `Header::validate` sur les chemins de lecture — est un
sous-ensemble strict de la même question normative.

**Rapport à l'étalon de juillet.** L'étalon publie déjà ce constat en clair sur
la branche publique `codex/audit-c-headers` (`af32734`), sous
`CHDR-015 — I3 is not enforced at the edition level — DECISION_REQUIRED, P2`.
Cette ronde le retrouve indépendamment, le relève à P1, et ajoute deux éléments
que juillet n'avait pas : le second vérificateur `publication::cold_verify`, et
le rattachement explicite à `spec/10-threat-model.md:19`.

**Référence de spec.** `spec/00-overview.md:33-34` ; `spec/03-headers.md:36-37` ;
`spec/10-threat-model.md:19` ; `spec/09-cli-and-conformance.md` §9.2.

**Critère de clôture.** Une décision enregistrée du propriétaire du protocole,
**antérieure** à toute correction, désignant laquelle des trois lectures fait
foi ; puis, selon cette décision, soit l'appel de `Header::validate` sur chaque
header de l'édition dans `Bundle::verify` **et** `publication::cold_verify`, soit
le resserrement de la phrase de spec, soit la validation sur les chemins de
lecture — et, dans les trois cas, un test qui échoue sur la baseline auditée pour
la raison nommée.

---

### `CHDR-012` — `DECISION_REQUIRED`, P2 — **0/3 réfutations**

**I3 est vérifié sur un champ que la spécification déclare non autorisant, et
non sur celui qu'elle déclare définitoire.**
**Scénario 5 / RU-2 — finding de surface publique.**

C'est le finding le plus solide du cycle : **aucun des trois réfuteurs ne l'a
entamé**, deux l'ont renforcé depuis des angles que le Pass A n'avait pas pris,
et il est **absent de l'étalon manuel de juillet**.

#### Le constat

Les quatre points de contrôle I3 de `aithos-core` comparent tous un **label** :

| Point de contrôle | Ligne | Test |
|---|---|---|
| `check_owner_line`, appelé par `build_at` (`header.rs:133`) et `rotate` (`:201`) | `header.rs:71-77` | `recipients.iter().any(\|r\| r.to == OWNER_LABEL)` |
| branche owner de `check_rotation` | `header.rs:298-303` | `new.lines.iter().any(\|l\| l.to == OWNER_LABEL)` |
| `validate` (parse-time) | `header.rs:308-315` | `kv.lines.iter().any(\|l\| l.to == OWNER_LABEL)` |

Or `spec/03-headers.md:33-35` déclare précisément ce champ non autorisant :

> `to` is a stable label (the grantee's multibase Ed25519 pubkey, or `"owner"`);
> it is **a routing hint only — the seal is what grants**.

Le commentaire de `header.rs:31-32` reprend la phrase mot pour mot. Les trois
champs de `Recipient` (`header.rs:16-18`) sont `pub`, donc le constructeur
`Recipient::owner` (`header.rs:22-28`) — le seul endroit où `to` et le `kid`
`"owner-kex"` sont posés ensemble — n'est en rien contraignant : n'importe quel
appelant peut construire un `Recipient { to: "owner", kid: …, pubkey: … }` à la
main.

#### Angle spec — l'écart est à la lettre, pas seulement à l'intention

`spec/01-identity-and-keys.md:23` définit :

> **owner_kex** is **the recipient key** of the owner's line in every header (I3).

La spécification définit donc la ligne owner **par sa clé destinataire**, pas par
son label. Le code vérifie l'inverse. Et la comparaison correcte est
**disponible et non faite** : à `build_at` et à `rotate`, `check_owner_line`
reçoit des `Recipient` qui portent un `pubkey: XPublicKey` (`header.rs:18`), et
`OwnerKeys::owner_kex_pub()` (`keys.rs:51-53`) rend exactement la valeur à
laquelle le comparer.

#### Angle modèle de menace — la garde correspondante n'existe pas

`spec/05-delegation.md:85-91` autorise explicitement un révocateur « owner **or
ancestor** » à re-sceller les lignes des survivants, **ligne owner comprise** :

> it rotates the node key and republishes the header omitting the revoked
> child's line but keeping every other line — including lines it did not create
> (those it re-seals under the new DK using its own access).

La règle de garde qui devrait borner ce pouvoir — un vérificateur rejette une
rotation de header dont le signataire n'est pas un émetteur autorisé — **n'est
pas implémentée**, ce que le dépôt constate déjà lui-même :
`docs/proposals/header-rotation-authority.md:37-48` relève que `check_rotation`
« ne vérifie que deux choses : aucun destinataire clandestin, la ligne owner est
présente. **Aucun contrôle d'autorité** », statut *Proposé — non adopté*.
Conséquence directe : un rotateur émettant `{ to: "owner", kid: <son propre kid,
déjà présent en v1> }` passe `check_rotation` — la garde anti-clandestin ne voit
rien puisque le `kid` existait, et la garde I3 ne voit rien puisque le label
dit `"owner"`.

#### Angle code — la seule liaison réelle est constructive, jamais vérificative

Le seul endroit du dépôt qui relie une ligne owner à la clé publiée dans le
document DID est `Bundle::owner_kex_recipient` (`grants.rs:171-174`) :

```rust
pub(crate) fn owner_kex_recipient(&self) -> Result<Recipient> {
    let doc = self.did_doc()?;
    let bytes = wire::multibase_to_x25519_pub(&doc.keys.kex)?;
    Ok(Recipient::owner(bytes.into()))
}
```

Il est **côté écrivain**. Aucune contrepartie vérificative n'existe, et il n'en
existe structurellement pas : `validate(&self)` et `check_rotation(&self, v)`
prennent le seul `Header` en paramètre et n'ont **aucun accès** au document DID.

#### Surface publique concernée

`aithos-cli/src/cmd/header_seal.rs:30-56` accepte des destinataires au format
libre `label:kid:x25519_pub_hex`, construit
`Recipient { to: label, kid, pubkey }` sans aucune contrainte sur `label`, et
les passe tels quels à `Header::build` (`:56`). En regard,
`aithos-cli/src/cmd/header_open.rs:27-32` valide puis ouvre — et **accepte** donc
le fichier ainsi produit, puisque `validate` ne regarde que le label.

#### Atténuations, relevées et pesées

1. `header_seal.rs:1-2` se déclare « DEV surface over test keys » : ce n'est pas
   une surface de production.
2. Une ligne owner falsifiée serait remplacée par la vraie à la rotation
   suivante : `revoke.rs:180`, `structure.rs:259` et `vault.rs:375` remplacent
   toute ligne dont `line.to == "owner"` par `owner_kex_recipient()`, c'est-à-dire
   par la clé du document DID. Le mensonge est donc auto-réparant à la première
   rotation — mais rien ne garantit qu'une rotation survienne, et ces trois sites
   **font confiance au même label** pour décider quelle ligne remplacer.

Ces atténuations réduisent l'exploitabilité ; elles ne touchent pas le constat,
qui est un écart entre la lettre de la spécification et le champ testé.

### Les deux lectures concurrentes — exposées, non arbitrées

| | Lecture A — la ligne owner est définie par sa clé | Lecture B — la ligne owner est définie par son label |
|---|---|---|
| Fondement | `spec/01-identity-and-keys.md:23` : `owner_kex` **est** « the recipient key of the owner's line » ; `spec/03-headers.md:33-35` retire toute autorité à `to` | I3 est un invariant **structurel** de l'objet header ; `to` est le champ que la structure expose, et lier I3 au document DID ferait sortir `Header` de `aithos-core`, qui ne connaît pas les DID |
| Conséquence | `check_owner_line` doit comparer `r.pubkey` à `owner_kex_pub()` ; `validate` et `check_rotation` doivent recevoir la clé attendue en paramètre | le code courant est correct, et c'est la couche appelante (`grants.rs:171-174` et ses homologues) qui porte la liaison |
| Coût | changement de signature de trois fonctions publiques de `aithos-core` ; `validate` cesse d'être `(&self)` | la spécification doit dire que `to` est *aussi* le champ définitoire de I3, ce qui contredit `spec/03-headers.md:33-35` |
| Porté par | l'auditeur et les trois réfuteurs | personne ne l'a défendue ; elle est reconstruite ici pour que la décision soit posée équitablement |

**Aucun correcteur ne peut choisir implicitement.** `DECISION_REQUIRED`,
propriétaire attendu : le propriétaire du protocole.

**Réconciliation.** Maintenu à P2, intact. C'est le seul finding de la ronde à
sortir du panel sans une seule réfutation. Le Pass B n'y a rien retiré et a
vérifié indépendamment chacune des références ci-dessus sur `a2087f2`.

**Référence de spec.** `spec/01-identity-and-keys.md:23` ;
`spec/03-headers.md:33-35`, `:36-37`, `:93-96` ; `spec/05-delegation.md:85-91` ;
`docs/proposals/header-rotation-authority.md:37-48`.

**Critère de clôture.** Une décision enregistrée du propriétaire du protocole
désignant le champ définitoire de I3 ; puis, si la lecture A est retenue,
comparer `r.pubkey` à `owner_kex_pub()` dans `check_owner_line` et donner à
`validate` / `check_rotation` la clé owner attendue — avec un test RED qui
construit un header portant `{ to: "owner", pubkey: <clé arbitraire> }`, passe
sur la baseline auditée, et échoue après correction.

---

### `CHDR-025` — `OPEN`, P2 — nouveau, issu de la passe d'état partagé

**La liaison `key_version` du sceau de ligne n'a aucun défenseur comportemental
dans le dépôt.**
**Scénario 4 et test de conformance C1.**

`c1_fail_closed` (`rust/crates/aithos-core/tests/c1_header_seal.rs:82-107`) est
le seul test négatif explicite de liaison de version du dépôt :

```rust
let other_ver = line_aad(&v.subject_did, &v.node, v.key_version + 1);
assert!(open_line(&sk, &epk, &c, &n, &other_ver).is_err());
```

Il n'a **aucun contrôle positif dans son propre corps**. Le triplet
`(sk, epk, c, n)` provient du vecteur ; que ce triplet s'ouvre sous l'AAD
nominale n'est établi que dans une *autre* fonction de test,
`c1_owner_and_grantee_lines` (`:76-80`). Toute mutation de `line_aad` change
l'AAD des deux côtés à la fois : l'assertion continue de passer, mais pour une
raison entièrement différente de celle que son commentaire nomme. Les trois
assertions sœurs de `c1_fail_closed` (`:92`, `:97`, `:101`) ont le même défaut.

Il ne reste alors, dans tout le dépôt, que des **épinglages d'octets** pour
défendre la composante `key_version` de `line_aad` (`seal.rs:29`, `:35-37`) :

- `c1_header_seal.rs:66-70` — `hex::encode(&c) == line.c_hex` contre
  `vectors/c1-header-seal.json` ;
- `g3_move.rs:149-152` — `hex::encode(line_aad(…)) == v.line_aad_hex` contre
  `vectors/g3-move.json`.

Et le premier de ces deux épinglages repose sur un vecteur dont **le générateur
n'existe pas dans le dépôt** : `c1_header_seal.rs:2-3` déclare « generated
independently (Python PyNaCl + manual RFC 5869 HKDF) », mais `vectors/` ne
contient aucun `gen-c1*` alors qu'il contient vingt-huit autres générateurs
`gen-*.py`. C'est exactement l'obligation `TARGETED` déjà enregistrée par la
revue d'impact acceptée de `b-derivation` ronde 2
(`features/.agents/orchestrator/runs/2026-08-03-b-derivation-impact-review-02.md:494`)
et reportée dans `features/.agents/c-headers/STATE.md`. Cette note en établit la
conséquence : ce n'est pas une simple classe de preuve à requalifier, c'est le
dernier verrou d'un invariant de sécurité de §3.8.

Côté Gherkin, le scénario 4 dit « bound to its node **and version** » et ne fait
varier que le nœud (`CHDR-001`). Les deux findings se composent : la moitié
« version » n'est ni exercée par le contrat, ni défendue comportementalement
ailleurs.

**Portée de la revendication.** Ce finding est établi par lecture du code
courant et par l'absence, vérifiée, de tout autre site. Il ne repose sur aucune
exécution : aucune expérience de mutation n'a été conduite par ce rôle, et la
mesure de rayon d'explosion publiée par l'étalon de juillet est écartée (§8).

**Référence de spec.** `spec/03-headers.md:32`, `:124-128` ;
`spec/00-overview.md:57-60`.

**Critère de clôture.** Donner à `c1_fail_closed` un contrôle positif dans son
propre corps — asserter d'abord que le tuple non modifié ouvre sur `dk_hex` —
de sorte que chacune des quatre assertions négatives soit un différentiel contre
une base connue bonne ; **et** produire ou retirer la revendication de
génération indépendante de `vectors/c1-header-seal.json`.

---

### `CHDR-001` — `OPEN`, P2 — 1/3 réfutations (survit)

**Le scénario « A line is bound to its node and version » n'exerce que la
liaison au nœud.**
**Scénario 4.**

`replay_line_other_node` (`cucumber.rs:8114-8122`) ne fait varier que le nœud :
`NODE_A` `/e/circle` → `NODE_OTHER` `/e/self`. Les deux `Header::build`
retombent sur la version 1 (`header.rs:114-116`) et l'ouverture se fait en
version 1 (`:8120`, `open_into(1, …)`). Les composantes `subject_did` et
`key_version` de `line_aad(subject_did, node, key_version)` (`seal.rs:35-37`,
`aad` `:21-31`), recalculées par `Header::open` (`header.rs:228`), sont
identiques des deux côtés. Le scénario prouve strictement moins que sa phrase.

**Correction imposée par le panel** (deux réfuteurs sur trois l'exigent) : ne pas
relayer ce constat en « la liaison à la version n'est testée nulle part ». Elle
l'est hors Gherkin — `c1_header_seal.rs:105-107` et l'épinglage d'octets
`g3_move.rs:149-152`. Le défaut est de **portée du scénario**, pas de couverture
du corpus.

**Réconciliation.** Maintenu, avec la correction du panel intégrée à l'énoncé
ci-dessus. Le Pass B ajoute une qualification que le panel n'avait pas :
`c1_header_seal.rs:105-107` est un défenseur **vacant** (`CHDR-025`). La
formulation exacte retenue est donc : *la liaison à la version n'est exercée par
aucun scénario, et hors Gherkin elle n'est défendue que par des épinglages
d'octets, jamais par un différentiel comportemental.* Le réfuteur dissident tient
que la conséquence de sécurité est nulle ; `CHDR-025` montre pourquoi elle ne
l'est pas.

**Référence de spec.** `spec/03-headers.md:32`, `:124-128`.

**Critère de clôture.** Une seconde tentative enregistrée qui greffe la même
ligne v1 dans une version 2 du **même** nœud (ou l'ouvre en version 2), les deux
tentatives devant être `Err` ; ou une scission du scénario en deux, pour que le
Gherkin cesse de revendiquer deux variations.

---

### `CHDR-009` — `OPEN`, P2 — 2/3 réfutations (réfuté, reformulé)

**Trois des quatre portails I3 du code n'ont aucun versant fail-closed testé, et
un cas spécifié par vecteur n'est pas implémenté.**
**Scénario 5.**

*Énoncé gelé au Pass A* : quatre portails I3 existent — `check_owner_line` sur
`build`/`build_at` (`header.rs:133`), sur `rotate` (`:201`), la branche owner de
`check_rotation` (`:298-303`), et `validate` (`:308-315`) ; le scénario n'exerce
que le premier.

*Réfutation majoritaire, vérifiée sur le code courant et acceptée* : les portails
2, 3 et 4 **sont exécutés**. `rotate` l'est par `cucumber.rs:8148` et `:15249` ;
la branche owner de `check_rotation` l'est par `g2_rotation.rs:92`
(`a_clean_rotation_is_accepted`) ;
`validate` l'est à chaque lecture de zone, vault, session et log
(`bundle.rs:630`, `:637`, `session.rs:363`, `log.rs:425`,
`aithos-cli/src/cmd/header_open.rs:28` — cinq sites). Ce qui manque est leur
**versant fail-closed**.

**Formulation exacte retenue après réconciliation :**

> Aucun test du dépôt n'assère `Error::MissingOwnerLine` ailleurs qu'au portail
> `build`. De plus `vectors/g2-rotation.json:17` déclare
> `"missing_owner_must_fail": "MissingOwnerLine"` — un cas normatif — que la
> struct `G2` de `rust/crates/aithos-core/tests/g2_rotation.rs:9-16` ne
> désérialise même pas : le champ n'a aucun consommateur dans le dépôt. Le cas
> est spécifié par le vecteur et n'est implémenté nulle part.

Vérifié sur le code courant : `vectors/g2-rotation.json:17` porte bien la clé ;
la struct `G2` déclare `old_kids`, `revoked_kid`, `expected_survivor_kids`,
`smuggled_new_kid`, `uplink` — et rien d'autre. Le champ frère
`smuggled_must_fail` (`:16`) est, lui, honoré par
`a_smuggled_recipient_is_rejected` (`g2_rotation.rs:68-80`) : l'asymétrie est
interne au même vecteur.

Second acquis du panel, retenu : la branche owner de `check_rotation` est
**dominée** par `check_owner_line` dans `rotate` chez ses deux appelants
(`revoke.rs:198-199`, `vault.rs:392-400`), `build_lines` recopiant `r.to`
verbatim (`header.rs:94-100`) — branche morte pour ces chemins, et même variante
d'erreur.

**Réconciliation.** Réfuté dans sa formulation gelée, **maintenu à P2 dans la
formulation ci-dessus**. La trouvaille du vecteur non désérialisé appartient au
panel et est portée au crédit de la ronde. Le finding change d'énoncé, pas de
sévérité : un cas normatif sans consommateur est plus grave qu'un portail non
exercé.

**Référence de spec.** `spec/03-headers.md:36-37` ; `spec/00-overview.md:33-35` ;
`spec/09-cli-and-conformance.md` §9.2.

**Critère de clôture.** Faire consommer `missing_owner_must_fail` par
`g2_rotation.rs` — désérialiser le champ, construire une v2 sans ligne owner et
asserter `Err(Error::MissingOwnerLine(_))` sur `check_rotation` — et ajouter une
assertion typée équivalente sur `rotate` et sur `validate`.

---

### `CHDR-013` — `OPEN`, P2 — 1/3 réfutations (survit)

**« Grant is one appended line » n'est asserté nulle part : ni cardinal, ni
position.**
**Scénario 6.**

`owner_line_untouched` (`cucumber.rs:12353-12361`) fait
`.find(|l| l.to == "owner")` puis `assert_eq!` contre l'instantané `saved_line`
(posé en `:7571` comme `lines[0].clone()`). Il ne lit ni `kv.lines.len()`, ni
l'index, ni l'ensemble des lignes. `append_line` fait un `push`
(`header.rs:180-186`) : la ligne owner d'origine reste à l'index 0 et `find` la
renvoie quoi que le mutant ait poussé ensuite. `Header::validate`
(`header.rs:308-315`) n'exige qu'**au moins une** ligne owner. Aucune assertion
sur un cardinal de lignes de header n'existe dans le dépôt. Le trou couvre aussi
une ligne surnuméraire vers une clé tierce, pas seulement un doublon.

**Réconciliation.** Maintenu à P2. Le réfuteur dissident objecte que le cardinal
figure au titre de la `Rule` (`features/c-headers.feature:38`) et non dans une
phrase du scénario, et que le grief appellerait donc un scénario supplémentaire
sous la `Rule` plutôt qu'une correction du scénario existant. La réconciliation
tranche : `PROCESS.md` § *Evidence hierarchy* point 1 fait du **scénario et de
ses exigences normatives citées** le contrat ; le titre de `Rule` est une
exigence normative citée, et `spec/03-headers.md:46-58` énonce la même chose.
Le grief vise le bon artefact. La remédiation, elle, peut légitimement prendre
la forme d'un scénario supplémentaire : le critère de clôture laisse les deux
ouvertes.

**Référence de spec.** `spec/03-headers.md:46-58`.

**Critère de clôture.** Une assertion de cardinal et de préfixe :
`lines.len() == saved.len() + 1` et égalité du préfixe contre l'instantané du
vecteur complet — ce qui épingle aussi l'ordre. Recouvre `CHDR-014`.

---

### `CHDR-014` — `OPEN`, P2 — 2/3 réfutations (réfuté, reformulé, maintenu)

**« Toute autre ligne intacte » est exercé sur un header dont « toute autre
ligne » a le cardinal 1.**
**Scénario 6.**

Le `Given` du scénario 6 est `sealed_header_owner_only` (`cucumber.rs:7569-7573`)
qui scelle à `&[owner_rec()]` — un seul destinataire. `key_versions["1"].lines`
tient donc exactement une entrée avant l'append. « Every other line untouched »
dégénère en « la seule autre ligne est intacte », et le scénario ne peut pas
distinguer un `push` `O(1)` d'un rebuild-and-reseal `O(n)` des destinataires
restants : avec `n = 1` il n'y a ni reste à perturber, ni ordre à permuter.

Le mutant qui re-scelle les lignes non-owner à l'append compile et passe :
`KeyVersion.lines` est `pub` (`header.rs:42-45`) et l'invariant n'est porté que
par un commentaire (`header.rs:157-158`).

*Clause du Pass A que le panel a réfutée, et qui est retirée de l'énoncé* : « le
fixture multi-destinataires n'est câblé qu'à la `Rule` de rotation ». C'est
faux. Il y en a **deux** : `sealed_header_owner_grantee` (`cucumber.rs:7553`,
owner + g1, câblé à `c-headers.feature:17` et `:22`, donc à la `Rule` « A line
seals… ») et `sealed_header_three` (`:7579`, câblé à `:49`). La remédiation en
est simplifiée, pas le finding.

**Réconciliation.** Réfuté 2/3, **maintenu à P2 après retrait de la clause
fausse**. Les deux réfutations attaquent des propositions annexes — le câblage
des fixtures, et la couverture fonctionnelle ailleurs (`cb10_structure_vault.rs`
`:307`/`:334`/`:355`, `cb9_delegated_content.rs:439`). Aucune n'atteint la
proposition centrale, qui est vérifiée sur le code courant : *ce* scénario
exerce l'invariant sur un ensemble de cardinal 1. La couverture ailleurs n'est
jamais byte-identique et plafonne à **une** ligne de grantee préexistante, ce
que le troisième réfuteur concède. `PROCESS.md` § *Evidence hierarchy* point 1
donne le contrat au scénario : une couverture ailleurs ne fait pas qu'un
scénario prouve sa phrase.

Le Pass B renforce ce maintien d'un fait différentiel : l'étalon manuel de
juillet porte le même grief (`CHDR-010` de juillet, P2) et l'a fait **survivre à
sa propre passe adverse** sur un code byte-identique (§8).

**Référence de spec.** `spec/03-headers.md:46-58`.

**Critère de clôture.** Pointer le `Given` sur un header à au moins deux
destinataires préexistants — `sealed_header_owner_grantee` existe déjà —,
instantanier le vecteur `lines` entier, appeler un grantee **différent**, et
asserter égalité de préfixe et cardinal. Recouvre `CHDR-013`.

---

### `CHDR-016` — `OPEN`, P2 — 1/3 réfutations (survit)

**Le chemin de grant de production n'implémente ni l'étape 1 ni l'étape 3 de
§3.3, et aucun pas de cette `Rule` ne le touche.**
**RU-3 — finding de surface publique.**

`Bundle::grant` (`grants.rs:739`) → `deliver_entry` (`:754`, corps `:308-341`) →
`add_line_on` (`:276-305`) :

- (a) calcule la DK par dérivation pure `node_key(&zone_dk, &node)`
  (`grants.rs:321`) sans jamais ouvrir le header existant — l'étape 1 de §3.3
  (« Open the node's current DK (own line) ») n'est pas exécutée ;
- (b) appelle `header.append_line(&did, KV, dk, …)` (`grants.rs:289`) avec
  `KV: u64 = 1` (`bundle.rs:25`) au lieu de `latest_version()`.

`rotate_folder` (`revoke.rs:142-240`) conserve v1 (`insert` de la clé « 2 »,
`header.rs:202-210`), scelle une DK' issue de `ent.e32()` (`revoke.rs:195`) et
bumpe chaque section à `key_version = 2`. Un `Bundle::grant` ultérieur sur ce
dossier dépose donc la ligne du nouveau lecteur dans `key_versions["1"]`,
scellant la clé dérivée pré-rotation ; le grant renvoie `Ok` et publie. Côté
lecture, `agent_section_key` demande v2 (`grants.rs:1037-1044`), l'ouverture
échoue faute de ligne, le repli `agent_node_key`/`try_header` ouvre à `KV`
(`grants.rs:827-830`) et rend une clé périmée, et `open_blob_v` à la version 2
refuse (`bundle.rs:505-518`). Fail-shut : le bénéficiaire reçoit moins que
prévu, le révoqué ne gagne rien.

Les deux surfaces conformes à §3.3 — `Session::append_header_recipient`
(`session.rs:354-366` : `validate`, puis `open_latest`, puis `append_line`) et
`deliver_connector_line` (`grants.rs:454-461`, `latest_version()`) — ne sont
touchées par aucun pas de cette `Rule`.

Deux précisions du panel, retenues : `deliver_exact_section` (`grants.rs:414`)
passe par `owner_current_section_key` et livre donc la bonne DK' — seule
l'étiquette de version de la ligne y est fausse ; et pour `move_folder` le
nouveau header est bâti par `build_at(new_v)` sans clé « 1 », donc le grant
échoue bruyamment — l'affirmation vaut pour la rotation de révocation.

**Réconciliation.** Maintenu à P2. Le réfuteur dissident classe le finding hors
périmètre au motif que `bundle.rs:25` porte le commentaire « single key version
**until step G** (revocation rotates) », donc dette assumée de
`g-revocation`/`d-bundle`. Le Pass B écarte cette réfutation **sur preuve de code
courant** : l'étape G a livré. `revoke.rs` existe, `rotate_folder`
(`revoke.rs:142-240`) tourne, `Header::build_at` existe pour le déplacement
(`header.rs:124-155`), et `grants.rs:1054-1070` lit déjà les wraps de rotation.
La condition suspensive du commentaire est échue et `KV = 1` est resté. Une
dette dont la date d'échéance est passée n'est plus une dette assumée.

Le finding reste néanmoins **à cheval sur deux périmètres** : le défaut vit dans
`aithos-bundle`, pas dans `aithos-core`. Il est consigné ici parce que
`PROCESS.md` § *Current scope* inclut explicitement « production surfaces that
bypass the exercised verdict », et il est **signalé comme impact** à
`g-revocation` et `d-bundle` (§9).

**Référence de spec.** `spec/03-headers.md:46-58` (§3.3, étapes 1 et 3) ;
`spec/03-headers.md:98-106` (§3.5, les lectures visent le verrou le plus récent).

**Critère de clôture.** Un pas de cette `Rule` qui traverse une surface de grant
de production conforme à §3.3, plus la correction de `add_line_on` pour qu'il
ouvre le header courant et append à `latest_version()`. La seconde moitié
appartient au cycle `g-revocation`/`d-bundle`.

---

### `CHDR-019` — `OPEN`, P2 — 1/3 réfutations (survit)

**« Le premier grantee ne peut pas ouvrir la nouvelle version » est décidé par
l'indice de routage, jamais par le sceau.**
**Scénario 7.**

`revoked_cannot_open` (`cucumber.rs:12375-12383`) appelle
`Header::open(DID_C, 2, "g1", &xsk(0x21))`. `Header::open` filtre
`kv.lines.iter().filter(|l| l.kid == kid)` (`header.rs:233`) ; la v2 construite
par le `When` (`cucumber.rs:8148-8161`) ne porte que les kids `owner-kex` et
`g2`. La boucle est donc **vide** et le contrôle tombe en `header.rs:242-245`
sans jamais appeler `open_line` (`seal.rs:110-132`). Le secret `xsk(0x21)` est
passé et n'est jamais utilisé. Le rejet est produit par un champ que
`spec/03-headers.md:33-35` déclare non-autorisant (« `to`/`kid` are routing
hints only — the seal is what grants »), commentaire repris en
`header.rs:31-32`.

Aucune assertion du scénario ne lit `key_versions["2"].lines` : le fait
structurel que la phrase énonce n'est jamais observé, et le fait cryptographique
n'est jamais exercé. Le scénario ne prouve ni l'un ni l'autre.

Le dépôt dispose de l'idiome fort à quatre règles de là : `stranger_tries`
(`cucumber.rs:8097-8102`) essaie tous les kids avec la même clé.

Régression survivante construite par un réfuteur, retenue : muter `kek`
(`seal.rs:83-89`) pour que l'IKM HKDF n'intègre plus le secret DH laisse le
nommage intact, `survivor_opens` et `owner_opens_new` verts, et rend la ligne
`g2` ouvrable par quiconque connaît la clé publique de g2. Autre mutant
survivant : une `rotate` recopiant les lignes v1 en v2 — la ligne `g1`
existerait mais échouerait sur l'AAD v2.

**Réconciliation.** Maintenu à P2. Le réfuteur dissident soutient que le titre du
scénario reprend `spec/03-headers.md:87` (« The revoked … gets no line in the new
version ») et que §3.2 définit « ouvrir » comme router puis desceller, l'absence
de ligne routable étant donc un échec d'ouverture au sens du contrat. La
réconciliation écarte cet argument sur la lettre du pas exécuté : la phrase du
`Then` est « the first grantee **cannot open** the new version », un énoncé de
capacité, pas de structure. Prouver la structure demanderait de lire `lines` ;
prouver la capacité demanderait d'atteindre le sceau. Le scénario ne fait ni
l'un ni l'autre.

**Portée élargie, signalée** : le même motif — kid du révoqué passé à
`open_latest` — se retrouve en `cucumber.rs:5013` et
`cb10_structure_vault.rs:548-553`.

**Référence de spec.** `spec/03-headers.md:33-35`, `:80`, `:87-89`, `:93-96` ;
`spec/06-revocation.md:25-44`.

**Critère de clôture.** Une assertion structurelle dans le `Then` existant —
`assert!(header.key_versions["2"].lines.iter().all(|l| l.kid != "g1"))` — et un
appel à `header.check_rotation(2)` au même endroit (voir `CHDR-024`).

---

### `CHDR-021` — `OPEN`, P2 — 1/3 réfutations (survit) — porte le verdict du scénario 8

**Le `Then` du wrap est un aller-retour sur lui-même et ne discrimine aucune
route.**
**Scénario 8.**

`post_uplink_wrap` (`cucumber.rs:8164-8175`) pose
`Wrap::seal(DID_C, NODE_A, &PARENT_KEY, CHILD_NODE, 2, &DK2, non(9))` dans
`w.wrap_obj` ; `parent_recovers_via_wrap` (`:12396-12404`) rouvre le **même objet
en mémoire** avec le **même littéral** `PARENT_KEY` et compare à `DK2`.
`Wrap::open` (`header.rs:351-357`) recalcule l'AAD depuis ses **propres** champs
`self.node` et `self.key_version` : l'assertion ne peut donc pas détecter un wrap
posté sous le mauvais nœud ni sous la mauvaise version.

Ce qui est établi est `wrap_open(wrap_seal(k, dk)) == dk`. Ne sont établis ni
(a) qu'un détenteur du parent atteignait l'enfant par dérivation avant la
rotation, ni (b) qu'il ne l'atteint plus après, ni (c) que la récupération passe
par le wrap plutôt que par une autre route — il n'en existe aucune autre,
`w.header` restant `None` pendant tout le scénario.

Mutants qui survivent, précisés par un réfuteur : toute mutation **symétrique**
de `aad()` (purpose `tagwrap` → autre, suppression des séparateurs `0x00`,
omission de `subject_did` / `wrapped_node` / `key_version`) et de
`derive_key(CTX_WRAP_KEY, ·)` (constante quelconque, dérivation réduite à
l'identité). Seul un mutant unilatéral meurt. Hors Gherkin, ces mutations
symétriques sont rattrapées par les épinglages d'octets `g3_move.rs:157-159`
(`wrap_aad_hex`) et `g2_rotation.rs:112-114` (`wrap.c == cipher_hex`) — ce qui
confirme que le scénario n'y contribue rien.

**Réconciliation.** Maintenu à P2, et **c'est ce finding qui porte le verdict
`SEMANTIC_FALSE_POSITIVE` du scénario 8** (§5). Le réfuteur dissident montre que
les cas négatifs du wrap et la restauration effective de la dérivation sont
couverts ailleurs — `c1_header_seal.rs:122`, `g-revocation.feature:65-69` et
`:76-79`, `g3_move.rs:157-159`. Argument de couverture, non de contrat : il ne
rend pas ce scénario honnête. Le même réfuteur formule une réserve que la
réconciliation retient et promeut en finding propre : il n'existe nulle part de
négatif **du wrap** par AAD divergente (`CHDR-026`).

**Référence de spec.** `spec/03-headers.md:69-84`, `:130-134` ;
`spec/02-content-tree.md` §2.5.

**Critère de clôture.** Le `Given` construit un état réel : dériver `K_P` pour le
parent, dériver la clé enfant pré-rotation par `node_key`, faire tourner une
vraie rotation du header enfant, ranger les deux. Le `Then` recouvre `K_P` par
dérivation depuis un ancêtre **avant** d'ouvrir le wrap, et assère en outre que
la clé enfant dérivée pré-rotation n'ouvre plus la nouvelle version — la paire
« coupée puis rétablie » que le nom du scénario revendique.

---

### `CHDR-002` — `OPEN`, P3 — 3/3 réfutations (réfuté, reformulé, déclassé)

**Les deux scénarios de rejet n'ont aucun contrôle positif interne.**
**Scénarios 3 et 4 (`Then` partagé).**

*Énoncé gelé au Pass A* : `opening_rejected` (`cucumber.rs:12342-12345`) sert deux
phrases et n'assère que `opened.last().unwrap().is_err()` ; les deux scénarios
prouveraient « une erreur est survenue », pas « le sceau a rejeté ».

*Réfutation unanime, vérifiée sur le code courant et acceptée* : les cinq sorties
d'erreur de `Header::open` (`header.rs:232`, `:234`, `:235`, `:237`, `:242`) sont
**toutes** la variante `Error::SealRejected` — asserter la variante serait une
tautologie ; et `open_into` fait `.map_err(|e| e.to_string())`
(`cucumber.rs:7402`), donc il n'existe plus de variante à examiner au moment du
`Then`. Surtout, dans les deux montages toutes les branches concurrentes sont
inatteignables : `corrupt_line` (`:8104-8112`) bascule un caractère hex en
préservant longueur et validité hex, la version 1 existe, `epk`/`n` sont intacts,
le kid correspond — seule la branche `:242` (échec du tag AEAD) est atteignable ;
`replay_line_other_node` (`:8114-8122`) insère une ligne bien formée de même kid
dans un header valide — idem. `is_err()` **est** donc « le sceau a rejeté ».

**Contre-preuve instruite au Pass B, qu'aucun réfuteur n'avait examinée.**
L'étalon de juillet formule un grief distinct sur le même `Then` : l'absence de
**contrôle positif**. Vérifié sur le code courant, il tient :

> Ni le scénario 3 ni le scénario 4 n'établissent que la ligne visée s'ouvrait
> *avant* la mutation. `sealed_header_owner_grantee` (`cucumber.rs:7553-7566`)
> et `sealed_header_owner_only` (`:7569-7573`) construisent le header et rien ne
> l'ouvre jusqu'au `When`, qui mute puis ouvre une seule fois. Une régression de
> fixture rendant la ligne owner définitivement inouvrable — un `owner_rec()`
> pointant sur une mauvaise clé publique, par exemple — laisserait les deux
> scénarios **verts**, l'assertion étant satisfaite pour une raison qui n'a rien
> à voir avec la corruption ni avec le rejeu.

**Formulation exacte retenue après réconciliation :**

> `opening_rejected` est un `is_err()` nu sans base connue bonne dans le corps du
> scénario. Les scénarios 3 et 4 ne sont pas différentiels : ils asserent l'échec
> après mutation sans avoir asserté le succès avant. Le seul contrôle positif de
> la `Rule` vit dans un **autre** scénario, `owner_opens` (`cucumber.rs:12312`,
> scénario 1).

**Réconciliation.** Réfuté 3/3 dans sa formulation gelée — la moitié
« attribution de cause » est **retirée**, le panel a raison. La moitié « contrôle
positif », distincte et non réfutée, est **maintenue et déclassée en P3** : sous
le code courant aucun mutant de production ne survit à la `Rule` entière grâce à
ce défaut, puisque `owner_opens` (scénario 1) tomberait. C'est un défaut de force
de preuve à l'échelle du scénario, pas un défaut vivant. Il se compose avec
`CHDR-027`.

**Référence de spec.** `spec/03-headers.md:29-32`, `:119-129`.

**Critère de clôture.** Rendre chaque scénario de rejet différentiel dans son
propre corps : dans `corrupt_line`, ouvrir une fois avant la bascule et une fois
après, asserter `opened[0] == Ok(DK)` puis `opened[1].is_err()` ; dans
`replay_line_other_node`, enregistrer une ouverture de contrôle de la ligne volée
sur son header d'origine avant la greffe.

---

### `CHDR-015` — `OPEN`, P3 — 3/3 réfutations (réfuté, reformulé, déclassé)

**La `Rule` du grant teste la primitive, pas la capacité de grant.**
**Scénario 6.**

*Énoncé gelé au Pass A* : l'étape 1 de §3.3 (« Open the node's current DK (own
line) ») n'est pas exercée ; le `When` passe la constante `DK` ; `append_line`
accepte un `dk` arbitraire sans vérifier qu'il est celui que scellent les lignes
présentes.

*Réfutation unanime, vérifiée sur le code courant et acceptée* : (i) mauvaise
attribution de couche — `append_line` (`header.rs:159-188`) ne détient aucun
secret X25519 et ne peut structurellement pas ouvrir une ligne ;
`session.rs:352-353` documente la frontière (« the DK and owner KEX secret never
cross this API boundary ») ; (ii) l'étape 1 **est** implémentée et exercée à sa
couche — `session.rs:364-365` (`open_latest` puis `append_line`),
`grants.rs:459-460`, `bundle.rs:631`/`:638`, et `e-mandates.feature:23-25`
traverse `Bundle::grant` puis l'agent déchiffre le contenu réel
(`cucumber.rs:9588`) ; (iii) la prémisse « la valeur que l'appelant lui a
scellée » est fausse — `assert_eq!(dk, DK)` (`cucumber.rs:12331`) compare à la
**constante de module** `DK = [0x77; 32]` (`:263`), un oracle de terrain, et
conjuguée au `Then` byte-identique elle épingle owner et grantee sur la même
valeur.

**Formulation exacte retenue après réconciliation :**

> La `Rule` « Grant is one appended line » n'exerce que la primitive
> `Header::append_line`. Aucun de ses pas ne traverse une capacité de grant de
> production — ni `Session::append_header_recipient` (`session.rs:354-366`), ni
> `Bundle::grant` (`grants.rs:739`). C'est une observation de **couverture** de
> la `Rule`, pas un défaut d'assertion du scénario.

**Réconciliation.** Réfuté 3/3, **déclassé en P3 et reformulé**. Le résidu concédé
par deux réfuteurs est exactement la formulation ci-dessus. Ce résidu recoupe
`CHDR-016`, qui porte la conséquence de sécurité ; `CHDR-015` n'en garde que la
part de couverture.

**Référence de spec.** `spec/03-headers.md:46-58`.

**Critère de clôture.** Fusion avec `CHDR-016` : un pas de la `Rule` qui traverse
une surface de grant conforme à §3.3 clôt les deux.

---

### `CHDR-020` — `OPEN`, P3 — 2/3 réfutations (réfuté, reformulé, déclassé)

**Le `Given` du scénario 8 est un corps vide : l'état composite qu'il nomme
n'est jamais construit.**
**Scénario 8.**

*Énoncé gelé au Pass A* : `derived_node_rotated` (`cucumber.rs:7598-7601`) a un
corps vide (un commentaire) ; les trois faits qu'il pose sont fictifs —
`CHILD_NODE` n'est relié à `PARENT_KEY` par aucune dérivation, aucune `rotate`
n'a lieu, `DK2` est une constante.

*Réfutations majoritaires, vérifiées sur le code courant* : (i) le `When`
reconstruit l'état énoncé sous forme d'arguments de `Wrap::seal` (via, via_key,
node, key_version = 2, dk) et écrit `w.wrap_obj` ; le `Given` vide est un idiome
récurrent du fichier, dont le jumeau `dk_and_two_recipients` (`:7548`) dans la
même feature ; (ii) les constantes ne sont pas arbitraires — vérifié :
`PARENT_KEY = [0x55; 32]` (`cucumber.rs:265`) est `vectors/g2-rotation.json:19`
(`via_key_hex = 5555…`), `DK2 = [0x66; 32]` (`:264`) est `:20`
(`new_dk_hex = 6666…`), `CHILD_NODE` est `:22`, la version 2 est `:23` ; et le
lien parent→enfant **est** cryptographique via `derive_key(CTX_WRAP_KEY, via_key)`
(`seal.rs:19`, `:137`), `CHILD_NODE` entrant dans `wrap_aad` (`seal.rs:39-42`).
Exiger une dérivation parent→DK' contredirait d'ailleurs
`spec/03-headers.md:66` (« Generate DK' (fresh random) … not derived from
anything he holds »).

**Nuance ajoutée par le Pass B.** L'alignement sur le vecteur G2 est partiel et
ne fait pas du scénario un contrôle de conformité : `vectors/g2-rotation.json:21`
fixe `nonce_hex = 7777…` là où le scénario passe `non(9) = [0x69; 24]`, et
`:24` fixe un `subject_did` différent de `DID_C`. Les deux entrent dans le calcul,
donc le chiffré du scénario ne peut pas égaler celui du vecteur, et aucune
assertion ne les compare.

**Formulation exacte retenue après réconciliation :**

> `derived_node_rotated` a un corps vide et ne place aucun état dans le `World`.
> Le texte du contrat est donc inexécutable : il peut être réécrit sans changer
> le résultat, et les entrées réelles du test ne sont pas lisibles depuis le
> contrat. Fidélité de contrat, au même titre que `CHDR-004` et `CHDR-010`.

**Réconciliation.** Réfuté 2/3, **déclassé en P3 et reformulé** en finding de
fidélité de contrat. La sévérité au niveau du scénario n'est pas perdue : elle
est portée par `CHDR-021`, qui survit, et qui établit le verdict
`SEMANTIC_FALSE_POSITIVE`. Les deux findings visent le même scénario et le
présent résultat les rend cohérents plutôt qu'opposés — le `Given` vide est le
**mécanisme**, le `Then` en aller-retour est la **conséquence**. Résidu concédé
par les réfuteurs et retenu : rien ne vérifie que `via` est ancêtre de `node`
(voir §9, impact `g-revocation`).

**Référence de spec.** `spec/03-headers.md:64-88`.

**Critère de clôture.** Que le `Given` place l'état qu'il nomme dans le `World` et
que le `When` le consomme. Subsumé par le critère de `CHDR-021`.

---

### `CHDR-004` — `OPEN`, P3

**`Given` vide : tout l'arrangement du scénario 1 vit dans son `When`.**
**Scénario 1.** Non soumis au panel (P3).

`dk_and_two_recipients` (`cucumber.rs:7548-7551`) a un corps vide. Le `When`
`seal_into_header` (`:8092-8095`) délègue à `sealed_header_owner_grantee`
(`:7553`), qui est aussi le `Given` des scénarios 2 et 3 : **le `When` du
scénario 1 et le `Given` des scénarios 2 et 3 sont le même code**. La séparation
`Given`/`When` du scénario 1 est fictive. Voir `CHDR-027` pour la conséquence
d'état partagé.

**Critère de clôture.** Que le `Given` pose `DK` et les deux `Recipient` dans le
`World` et que le `When` les consomme.

---

### `CHDR-005` — `OPEN`, P3

**« every line » n'est contraint par aucune assertion du scénario 2.**
**Scénario 2.** Non soumis au panel (P3).

`stranger_recovers_nothing` (`cucumber.rs:12335-12339`) vérifie
`!opened.is_empty()` puis `all(is_err)` : le nombre de tentatives n'est pas fixé.
Le mot « every » n'est porté que par la boucle du `When` sur le littéral
`["owner-kex", "g1"]` (`:8098`), pas par les lignes du header. Une ligne ajoutée
au fixture ne serait pas essayée ; un kid littéral cessant de correspondre
produirait un `Err` vide depuis `header.rs:242` sans aucun déchiffrement. Sous le
code actuel la couverture est en fait complète : défaut de force de preuve, pas
défaut vivant.

**Critère de clôture.** Dériver la liste de kids du header, et asserter
`opened.len() == lines.len()` à côté de `all(is_err)`.

---

### `CHDR-006` — `OPEN`, P3

**Aucun scénario n'épingle la bijection destinataire → ligne, et le constructeur
tronque silencieusement.**
**Scénario 1.** Non soumis au panel (P3).

`build_lines` (`header.rs:83-102`) zippe `recipients` avec
`ephemerals.iter().zip(nonces)` : un `zip` tronque à la plus courte des trois
séquences, sans erreur. Aucun scénario n'assère que le nombre de lignes égale le
nombre de destinataires.

**Recoupe `CHDR-023`, requalifié hors périmètre (§7).** La différence est que
`CHDR-023` visait `Header::rotate` — dont les deux appelants construisent des
cardinalités égales par construction — tandis que `CHDR-006` vise une assertion
que le scénario 1 pourrait porter et ne porte pas. `CHDR-006` reste donc un
finding, à P3, dans le périmètre de la vérité sémantique du scénario 1.

**Critère de clôture.** Une assertion de cardinal dans le `Then` du scénario 1.

---

### `CHDR-010` — `OPEN`, P3

**`Given` vide : les paramètres de la phrase ne sont jamais liés.**
**Scénario 5.** Non soumis au panel (P3).

`single_grantee` (`cucumber.rs:7576`) a un corps vide. Ni « a node key » ni « a
single grantee recipient » ne deviennent un état ; le `When`
`build_without_owner` (`:8124-8137`) recrée tout depuis des constantes de
compilation.

**Critère de clôture.** Identique à `CHDR-004`.

---

### `CHDR-011` — `OPEN`, P3

**Le `Then` n'assère qu'une sous-chaîne du message d'erreur.**
**Scénario 5.** Non soumis au panel (P3).

`build_without_owner` (`cucumber.rs:8134`) range `e.to_string()`, détruisant
l'erreur typée à la frontière du `World` ; `header_invalid` (`:12347-12351`)
assère ensuite `msg.contains("I3")`. La variante typée `Error::MissingOwnerLine`
est publique et `Error` dérive `PartialEq`. Ni la variante ni le nœud transporté
(`/e/circle`) ne sont vérifiés : la discrimination tient par coïncidence
lexicale entre `error.rs:59` et `:71`. Un chemin de nœud contenant le littéral
`I3` satisferait l'assertion sans que le contrôle owner soit la cause — la charge
utile est `node.to_owned()` (`header.rs:75`).

Le scénario reste néanmoins fail-closed : le `When` panique sur `Ok`
(`cucumber.rs:8133`) et l'`unwrap()` du `Then` sur un `rejection` à `None`
échoue indépendamment.

**Note d'état partagé.** `rejection` (`cucumber.rs:463`) est un champ du `World`
partagé par tout le fichier de 19 700 lignes : il est écrit en `:7796` et
`:8134`, lu en `:12348` et `:12513`. Le `World` étant réinstancié par scénario
(§7), aucune valeur ne traverse un scénario aujourd'hui. Le jour où un `Given`
de `c-headers` écrirait `rejection`, l'assertion par sous-chaîne cesserait d'être
discriminante sans que rien ne le signale.

**Critère de clôture.** Ranger l'erreur typée et asserter
`matches!(err, Error::MissingOwnerLine(ref n) if n == NODE_A)`.

---

### `CHDR-017` — `OPEN`, P3

**La revendication `O(1)` du récit de la `Feature` n'est ni mesurée ni assertée.**
**Scénario 6.** Non soumis au panel (P3).

`features/c-headers.feature:5` et `spec/03-headers.md` §3.3 revendiquent `O(1)`.
La seule trace est structurelle : `append_line` fait un `push` (`header.rs:180`)
et ne lit aucun champ d'aucune ligne existante (`:159-188`). C'est une preuve de
code, pas une preuve de scénario.

**Critère de clôture.** Soit une assertion structurelle (préfixe intact,
cardinal +1 — voir `CHDR-013`), soit le retrait de la revendication du récit.

---

### `CHDR-018` — `OPEN`, P3

**Le `Then` est une fonction partagée et câblée en dur, incapable de distinguer
une ligne appendue d'une ligne construite.**
**Scénarios 1 et 6.** Non soumis au panel (P3).

`grantee_opens` (`cucumber.rs:12324-12333`) porte deux phrases `#[then]` de deux
`Rule` différentes — « the grantee opens the header and recovers the node key »
(`features/c-headers.feature:14`) et « the new grantee opens the node key »
(`:43`) — et code en dur version 1, kid `g1`, secret `xsk(0x21)`, attendu `DK`.
Le mot « new » de la seconde phrase n'a aucun correspondant dans le code : la
fonction ne peut pas distinguer la ligne appendue par le scénario 6 de la ligne
construite par le scénario 1.

**Critère de clôture.** Deux fonctions distinctes, ou un paramètre Gherkin lié.

---

### `CHDR-024` — `OPEN`, P3

**Aucun pas de RU-4 n'appelle `check_rotation` : la bonne forme mécanique de
§3.4 est hors de portée de la `Rule` qui la nomme.**
**Scénarios 7 et 8.** Non soumis au panel (P3).

`Header::check_rotation` (`header.rs:275-305`) implémente exactement la bonne
forme que le titre de la `Rule` revendique. `Header::rotate` (`:192-217`) ne
l'appelle pas — il n'appelle que `check_owner_line` (`:201`). Ses appelants
vérifiés, exhaustivement, sur le code courant : `revoke.rs:199`, `vault.rs:400`,
`cucumber.rs:15260` (un pas de `g-revocation`), `g2_rotation.rs:79` et `:92`.
Aucun n'appartient à `c-headers`.

**Hors verdict, consigné pour l'intégration.** `check_rotation` est lui-même plus
faible que `spec/03-headers.md:93-96`, qui exige une **égalité** « previous minus
revoked » là où `header.rs:288-297` ne teste qu'une **inclusion** : une rotation
qui *supprime* un survivant sans autorité passe. Déjà connu du dépôt
(`docs/proposals/header-rotation-authority.md:37-48`, statut *Proposé — non
adopté*). Ce point n'est énoncé par aucun scénario de `c-headers` : il est
signalé, non audité.

**Recoupe `CHDR-009`** (portails I3) et l'étalon de juillet (§8).

**Critère de clôture.** Invoquer `check_rotation(2)` dans le `Then` existant du
scénario 7. Recouvre la moitié structurelle de `CHDR-019`.

---

### `CHDR-026` — `OPEN`, P3 — nouveau, issu de la passe d'état partagé

**Le wrap n'a aucun négatif par AAD divergente, nulle part.**
**Scénario 8.**

Le sceau de ligne dispose de négatifs sur les deux axes de son AAD —
`c1_header_seal.rs:100-102` (autre nœud) et `:105-107` (autre version) — même si
`CHDR-025` établit qu'ils sont vacants. Le wrap n'en a **aucun**. Recensement
exhaustif des sites qui exercent `wrap_open` ou `Wrap::open` dans le dépôt :

| Site | Ce qu'il assère |
|---|---|
| `c1_header_seal.rs:117-119` | aller-retour sous la bonne clé |
| `c1_header_seal.rs:122` | échec sous une **clé via** nulle — seul négatif du wrap |
| `g2_rotation.rs:112-116` | octets contre le vecteur, puis aller-retour |
| `g3_move.rs:157-176` | `wrap_aad` épinglé, puis aller-retour sous la nouvelle clé parent |
| `cucumber.rs:12401` | le `Then` du scénario 8 |
| `grants.rs:1054`, `:1063` | chemin de lecture de production |

`wrap_aad` est épinglé octet à octet (`g3_move.rs:157-159`), mais aucun test
n'établit qu'un `Wrap` posté sous un autre nœud ou une autre version est refusé.
`Wrap::open` recalculant son AAD depuis ses propres champs (`header.rs:351-353`),
un `Wrap` dont `node` ou `key_version` aurait été réécrit par un attaquant
échouerait — mais rien ne le prouve, et l'asymétrie avec le sceau de ligne est
non intentionnelle.

**Référence de spec.** `spec/03-headers.md:72-84`, `:130-134`.

**Critère de clôture.** Deux assertions dans `c1_header_seal.rs::c2_wrap_…` :
rouvrir le chiffré du vecteur sous `wrap_aad(did, autre_nœud, version)` puis
sous `wrap_aad(did, nœud, version + 1)`, les deux devant être `Err`, après un
contrôle positif dans le même corps (voir `CHDR-025`).

---

### `CHDR-027` — `OPEN`, P3 — nouveau, issu de la passe d'état partagé

**Toute la `Rule` RU-1 repose sur un unique constructeur de fixture, et son seul
contrôle positif vit dans un autre scénario que ceux qui en dépendent.**
**Scénarios 1 à 4.**

Trois des quatre scénarios de RU-1 partagent le même constructeur :
`sealed_header_owner_grantee` (`cucumber.rs:7553-7566`) est le `Given` des
scénarios 2 et 3 **et** le corps du `When` du scénario 1 (`:8092-8095`, via
`CHDR-004`). Le quatrième, le scénario 4, utilise
`sealed_header_owner_only` (`:7569-7573`), qui porte deux phrases `#[given]` —
`features/c-headers.feature:27` (scénario 4) et `:41` (scénario 6, une autre
`Rule`) — et écrit **deux** champs du `World`, `saved_line` et `header` ; le
scénario 4 reçoit donc un instantané `saved_line` qu'il ne lit jamais.

La `Rule` entière ne comporte ainsi que deux formes d'appel à `Header::build`, et
un unique contrôle positif : `owner_opens` (`:12312-12322`), dans le scénario 1.
Les scénarios 3 et 4 asserent un rejet sans jamais avoir établi une base connue
bonne dans leur propre corps (`CHDR-002`) : leur pouvoir de détection est donc
emprunté à un scénario voisin. Ce n'est pas un `PROXY` au sens du tableau des
statuts — aucun verdict partagé n'est consommé — mais c'est un couplage que
l'isolation par unité de revue ne pouvait pas voir, et c'est précisément ce que
le point 5 de `PROCESS.md` § *Review-unit isolation* demande d'instruire.

**Critère de clôture.** Un contrôle positif interne dans chacun des scénarios 3
et 4 (voir `CHDR-002`), ce qui rompt la dépendance.

## 7. Findings retirés ou requalifiés

| Id | Panel | Décision de réconciliation | Motif, sur preuve de code courant |
|---|---|---|---|
| `CHDR-003` | 2/3 réfuté | **retiré** — embargo levé avec le finding | voir ci-dessous |
| `CHDR-008` | 2/3 réfuté | **retiré** en tant que finding autonome — absorbé par `CHDR-007`, publié en entier en §6 | voir ci-dessous |
| `CHDR-022` | 1/3 réfuté (survivait) | **requalifié en impact** `g-revocation` (§9) — n'est plus un finding `c-headers` | voir ci-dessous |
| `CHDR-023` | 3/3 réfuté | **requalifié hors périmètre** — durcissement défensif | voir ci-dessous |

### `CHDR-003` — retiré, embargo levé

*Titre neutre du gel : « Actual reach of the node binding proved by the replay
scenario ».* L'embargo tombant avec le finding, l'énoncé est publié.

*Énoncé gelé* : `Header::open` (`header.rs:228`) construit l'AAD depuis
`self.node`, le champ auto-déclaré du fichier désérialisé, et non l'emplacement
de stockage ; la liaison prouvée par le scénario 4 serait « ligne ↔ champ node »
et non « ligne ↔ nœud d'appartenance ».

*Réfutation confirmée par le Pass B sur trois preuves de code courant, chacune
revérifiée indépendamment :*

1. **Le verrou est structurel et ailleurs.** `hdr_file`
   (`grants.rs:139-146`) place tout header à
   `e/<zone>/hdr/<blake3(node)[..12]>.json`. Déplacer un header sous un autre
   nœud sans que le lecteur s'en aperçoive exigerait une collision BLAKE3 sur
   96 bits.
2. **L'AAD des blobs ne vient jamais de `header.node`.** `open_blob_v`
   (`bundle.rs:504-518`) calcule `blob_aad(&self.did, &node.to_string(),
   version)` depuis le `NodePath` **résolu par l'appelant**, comme `seal_blob`
   (`:492`). Un header déplacé rend donc la DK de son nœud d'origine, laquelle
   n'ouvre aucun blob du nœud cible : fail-shut.
3. **L'ancrage Merkle indexe le hash du header par son chemin.**
   `vault_build` (`state.rs:240-248`) et `header_hash_at` (`:58-62`) associent
   `BLAKE3(JCS(header.json))` à `path` ; `manifest.files` épingle
   path → sha256 ; le tout est signé.

Un contrôle explicite chemin ↔ champ existe par ailleurs, en `vault.rs:114-119`
(`header.node != Self::config_node(connector)` → `Error::SealRejected`), ce qui
confirme que l'idiome est connu du dépôt et employé là où il est nécessaire.

Le réfuteur dissident objectait qu'aucun contrôle équivalent n'existe aux autres
sites de lecture et que `read_vault_config_owner` (`vault.rs:335`) ne recoupe pas
non plus. C'est exact, mais sans conséquence : les points 1 à 3 rendent la
substitution soit impossible, soit inoffensive. **Aucune preuve de code courant
ne soutient une conséquence de sécurité. Le finding est retiré.**

### `CHDR-008` — retiré en tant que finding autonome

*Titre neutre du gel : « Coverage of parse-time I3 validation across header read
paths ».*

Ce finding portait sur la **couverture** de `Header::validate` sur les chemins de
lecture. Sa base factuelle est vérifiée et n'est pas contestée : `Header` dérive
`Deserialize` sans hook (`header.rs:47`), cinq sites seulement appellent
`validate()` — `bundle.rs:630`, `:637`, `log.rs:425`, `session.rs:363`,
`aithos-cli/src/cmd/header_open.rs:28` — tandis que `Header` est désérialisé sur
bien plus de sites, dont `grants.rs:287`, `:456`, `:827`, `:1037`, `:1197`,
`structure.rs:199`, `:751`, `revoke.rs:289`, `:365`, `:510` et `bundle.rs:670`.
`append_line` (`header.rs:159-188`) ne refait pas `check_owner_line`.

Les deux réfutations acceptées : (i) I3 est une propriété de **disponibilité**,
non de confidentialité (`spec/10-threat-model.md:19`), et aucun site non validant
ne produit de résultat faux — `Header::open` échoue fail-shut ; (ii) cinq des
sept chemins qui **mutent** un header portent un contrôle I3 équivalent via
`rotate` ou `build_at`, si bien que le trou réel se réduit à `add_line_on`
(`grants.rs:287-291`), et l'asymétrie propriétaire/délégué invoquée par le Pass A
est fausse en général — `bundle.rs:670` est un chemin propriétaire non validant.

Le Pass B constate que cet énoncé est **un sous-ensemble strict** de `CHDR-007`,
dont il partage la question normative et la décision attendue. Le conserver comme
finding autonome dédoublerait la même décision humaine et lui donnerait deux
critères de clôture concurrents. Il est donc **retiré et absorbé par
`CHDR-007`**, dont il devient une pièce de dossier. Sa matière est publiée
ci-dessus, l'embargo ayant été levé sur les deux identifiants le 2026-08-03
(§6, préambule).

Consigné pour le propriétaire de la décision : le réfuteur dissident (angle
périmètre) n'a pas pu réfuter et a signalé que
`features/.agents/c-headers/auditor/audit-c-headers/SKILL.md:52` et `:76`
**commandent nommément** cette analyse. Le retrait est un choix de structure du
dossier, pas un abandon de l'analyse.

### `CHDR-022` — requalifié en impact `g-revocation`

*Énoncé gelé* : le `via` modélisé par le scénario 8 ne correspond pas à celui que
la surface de rotation réelle poste.

Le Pass B **conteste l'énoncé sur la topologie du scénario lui-même**.
`NodePath::zone_root(Zone::Circle).to_string()` vaut `/e/circle`
(`path.rs:59-65`, `:135-147`, `Zone::as_str` `:20-26`) — c'est exactement
`NODE_A`. Et `CHILD_NODE = /e/circle/d/000…01` est de profondeur 1 : son parent
direct **est** la racine de zone. Le scénario modélise donc précisément ce que
`rotate_folder` poste (`revoke.rs:204-214` : `via = NodePath::zone_root(zone)`,
clé `zone_dk`). À sa propre profondeur, le scénario ne diverge pas de la
production.

Ce que le finding établit réellement, et qui est vrai sur le code courant, est un
défaut de **disponibilité en production à profondeur ≥ 2** : `rotate_folder`
poste toujours l'up-link sous la racine de zone, et `agent_section_key` ne tente
ce wrap que si `depth == 0` (`grants.rs:1061-1070`, commentaire « only the
zone-root key itself opens them » ; `structure.rs:216` répète le garde). Un
détenteur du parent intermédiaire entre la boucle à `depth = 1`, cherche
`wrap_file([a], [a,b])` qui n'existe pas, ne peut pas prendre la branche racine,
retombe sur `node_key` (`grants.rs:1071-1078`) et obtient une clé périmée — alors
que `spec/03-headers.md:76-80` promet que « holders of P (or of any ancestor of
P) keep reading N by derivation ». S'y ajoute que `agent_section_key` s'arrête au
**premier** header ouvrable et retourne (`grants.rs:1080-1082`) sans réessayer un
ancêtre plus haut.

Ce défaut vit entièrement dans `aithos-bundle`, ne se manifeste à aucune
profondeur atteinte par un scénario de `c-headers`, et relève de
`g-revocation`. Les limites du pilote (`DOMAIN.md` § *Pilot limits*) sont
explicites : ce qui touche `g-revocation` est **un impact à signaler, pas un
finding à auditer**. Il est donc requalifié et reporté en §9. La dette est déjà
consignée dans `docs/archive/HANDOFF.md:449`.

### `CHDR-023` — requalifié hors périmètre

*Énoncé gelé* : `Header::rotate` valide `check_owner_line` sur la liste de
destinataires puis délègue à `build_lines` dont le `zip` tronque silencieusement
(`header.rs:89-102`) — fail-open possible sur I3 ; et `key_versions.insert`
(`header.rs:202`) n'exige ni monotonie ni absence de la clé.

*Réfutation unanime, confirmée sur le code courant* : les deux cas sont
inatteignables. Les deux seuls appelants construisent éphémères et nonces par
`survivors.iter().map(…)` (`revoke.rs:196-197`, `vault.rs:389-390`) —
cardinalités égales par construction — et calculent `new_v = latest_version() + 1`
(`revoke.rs:156-157`, `vault.rs:387-388`), strictement croissant. Chacun appelle
`check_rotation` immédiatement après (`revoke.rs:199`, `vault.rs:400`), qui
revérifie la ligne owner **sur les lignes produites** (`header.rs:298-303`) ; et
`validate` rejoue I3 à chaque parse.

De surcroît, aucun scénario de `c-headers` n'énonce cette propriété :
`PROCESS.md` § *Current scope* exclut « general searches for behavior not
described by an existing scenario ». **Requalifié en durcissement défensif hors
périmètre.** Consigné, non audité, sans critère de clôture.

## 8. Comparaison à l'étalon manuel de juillet

L'étalon est `docs/audits/features/c-headers.md` de la branche publique
`origin/codex/audit-c-headers` (`af32734`), daté du 2026-07-30, révision observée
`3803fe8`, seize findings `CHDR-001`…`CHDR-016` numérotés dans un **espace de
noms distinct** de celui de la présente note (§1).

### 8.1 Le code audité est identique

Diff `3803fe8..a2087f2` sur le périmètre :

| Fichier | Diff |
|---|---|
| `features/c-headers.feature` | identique |
| `rust/crates/aithos-core/src/header.rs` | identique |
| `rust/crates/aithos-core/src/seal.rs` | identique |
| `rust/crates/aithos-core/tests/c1_header_seal.rs` | identique |
| `rust/crates/aithos-core/tests/g2_rotation.rs` | identique |
| `rust/crates/aithos-core/tests/g3_move.rs` | identique |
| `vectors/c1-header-seal.json` | identique |
| `vectors/g2-rotation.json` | identique |
| `rust/crates/aithos-bundle/tests/cucumber.rs` | 16 insertions, 3 suppressions — **uniquement `main()`**, le correctif `BDER-011` |

Aucune définition de pas, aucun fixture, aucun champ du `World`, aucun helper
n'a bougé. La comparaison n'a donc **aucune excuse de dérive** : un finding de
juillet non retrouvé cette ronde est un manqué, pas une observation périmée.

### 8.2 Ce que les preuves de gate de juillet valent

Rien. La branche étalon part de `240c658`, antérieur au correctif `BDER-011` :
son `main()` appelait `filter_run`, qui sous `harness = false` sort `0` même avec
des scénarios en échec. L'étalon le dit lui-même. **Aucun chiffre de gate de
juillet n'est cité dans cette note.**

Une revendication d'exécution de juillet est en outre **contredite par le code
courant**, et le fait est consigné parce qu'il touche `CHDR-025`. L'étalon
rapporte qu'une mutation retirant `key_version` de `line_aad` laissait
« 18 features / 836 scenarios / 3577 steps » verts, la seule défaillance de tout
le workspace étant `c1_owner_and_grantee_lines`. Or `g3_move.rs:149-152` assère
`hex::encode(line_aad(&v.subject_did, &v.new_node, v.key_version)) ==
v.line_aad_hex`, et ce fichier est **identique** entre les deux révisions ; son
dernier commit, `97d7187`, est un ancêtre de `240c658`. Cette assertion aurait dû
tomber elle aussi. Ce rôle n'exécute aucune commande et ne peut donc pas
trancher par mesure : le fait est consigné comme une contradiction entre une
revendication d'exécution non reproduite et la lecture du code courant, et la
revendication est écartée. `CHDR-025` ne s'appuie que sur la lecture.

### 8.3 Table de correspondance — findings P1/P2 de juillet

Neuf findings de juillet sont P1 ou P2.

| Juillet (`af32734`) | Sév. juillet | Retrouvé seul cette ronde ? | Identifiant 2026-08-03 | Sév. | Écart |
|---|---|---|---|---|---|
| `CHDR-001` — le scénario du wrap ne prouve rien de ce qu'il revendique (sc. 8, `SEMANTIC_FALSE_POSITIVE`) | P1 | **oui** | `CHDR-021` (+ `CHDR-020`, `CHDR-026`) | P2 | sévérité abaissée P1 → P2 ; verdict de scénario identique après réconciliation |
| `CHDR-002` — « gets no line » prouvé comme « cannot open » (sc. 7) | P1 | **oui** | `CHDR-019` | P2 | sévérité abaissée P1 → P2 |
| `CHDR-003` — `check_rotation` n'est appelé par aucun pas de la `Rule` | P2 | **oui** | `CHDR-024` | P3 | sévérité abaissée P2 → P3 |
| `CHDR-004` — l'assertion « revoked cannot open » survit à la rotation qui n'a pas lieu | P2 | **non** | — | — | **manqué** (§8.4) |
| `CHDR-006` — la moitié « version » du scénario de liaison n'est jamais exercée (sc. 4) | P2 | **oui** | `CHDR-001` | P2 | identique |
| `CHDR-007` — les assertions de rejet n'attribuent aucune cause et n'ont aucun contrôle positif (sc. 3 et 4) | P2 | **partiellement** | `CHDR-002` | P3 | la moitié « cause » est réfutée 3/3 et retirée ; la moitié « contrôle positif » n'a été retrouvée qu'au Pass B, **en lisant l'étalon** — pas seule |
| `CHDR-010` — « touching nobody » exercé sur un header à une ligne (sc. 6) | P2 | **oui** | `CHDR-014` | P2 | identique ; réfuté 2/3 par le panel, rétabli en réconciliation |
| `CHDR-015` — I3 n'est pas imposé au niveau de l'édition (`DECISION_REQUIRED`) | P2 | **oui** | `CHDR-007` | P1 | sévérité **relevée** P2 → P1 ; cette ronde ajoute le second vérificateur `publication::cold_verify` (`publication.rs:836-939`) et le rattachement à `spec/10-threat-model.md:19`, absents de l'étalon. Un embargo avait été posé sur ce constat déjà publié par l'étalon ; il a été levé par décision du propriétaire le 2026-08-03 (§6, §15) |
| `CHDR-016` — le seul test qui garde la liaison de version la garde vacuement | P2 | **non** | `CHDR-025` (Pass B) | P2 | **manqué au Pass A** ; retrouvé au Pass B, indépendamment renforcé par l'absence de générateur `gen-c1*` |

### 8.4 Manqués — chiffres bruts

**Deux findings P1/P2 de juillet ont échappé au Pass A de cette ronde.**

- `CHDR-004` de juillet — **non retrouvé, à aucun stade.** L'assertion
  `revoked_cannot_open` (`cucumber.rs:12375-12383`) n'assère que `is_err()`. Si
  le `When` (`:8148`) était supprimé ou neutralisé, `key_versions` ne porterait
  aucune clé « 2 » et `Header::open` renverrait
  `Error::SealRejected("no key version 2")` en `header.rs:229-232` — **et ce
  `Then` passerait encore**. Il n'est protégé que par ses deux `Then` frères, qui
  font `unwrap()` sur la version 2. `CHDR-019` de cette ronde décrit la branche
  `header.rs:242-245` (boucle de kids vide) et **pas** la branche `:229-232`
  (version absente). Le manqué est réel et distinct. Vérifié sur le code courant.
  Il est absorbé par le critère de clôture de `CHDR-019`, qui exige une assertion
  structurelle établissant la précondition de version 2 — mais il n'a pas été
  trouvé par ce cycle.
- `CHDR-016` de juillet — **manqué au Pass A**, y compris par les seize
  réfuteurs. Pire : le panel a **utilisé** `c1_header_seal.rs:105-107` comme
  preuve de code courant pour imposer une correction de `CHDR-001`, c'est-à-dire
  s'est appuyé sur le test même que juillet avait montré vacant. Le Pass B a
  retrouvé le fait et l'a promu en `CHDR-025`, avec une preuve supplémentaire que
  juillet n'avait pas : l'absence de générateur `gen-c1*` dans `vectors/`.

Deux findings P3 de juillet sont également sans équivalent cette ronde et sont
consignés sans être promus : `CHDR-005` (les deux moitiés de la `Rule` de rotation
ne sont jamais jointes — les scénarios 7 et 8 visent des nœuds différents) et
`CHDR-009` (aucun scénario n'atteint les vecteurs C1/C2 ; `c1_header_seal.rs` ne
construit jamais de `Header`).

### 8.5 Nouveaux — ce que juillet n'avait pas

| Cette ronde | Sév. | Nature |
|---|---|---|
| `CHDR-012` | P2 | absent de l'étalon ; **0/3 réfutation** — le seul finding de la ronde à sortir du panel intact ; `DECISION_REQUIRED` |
| `CHDR-016` | P2 | le chemin de grant de production (`Bundle::grant` → `add_line_on`) appende à `KV = 1` après rotation ; absent de l'étalon |
| `CHDR-013` | P2 | cardinal et position des lignes après append — juillet le portait à P3 (`CHDR-012` de juillet), cette ronde à P2 |
| `CHDR-009` | P2 | le cas `missing_owner_must_fail` de `vectors/g2-rotation.json:17` n'a aucun consommateur — **trouvaille du panel de réfutation**, absente de l'étalon |
| `CHDR-022` (requalifié) | — | la divergence de `via` de `rotate_folder` à profondeur ≥ 2 ; absente de l'étalon ; reportée en impact `g-revocation` |
| `CHDR-026` | P3 | aucun négatif du wrap par AAD divergente ; absent de l'étalon |
| `CHDR-027` | P3 | couplage de fixture de RU-1 et localisation du seul contrôle positif ; absent de l'étalon |

### 8.6 Chiffres bruts

| Mesure | Juillet (`af32734`) | Cette ronde (`a2087f2`) |
|---|---|---|
| Findings publiés | 16 | 27 identifiants, dont 23 findings actifs |
| P1 | 2 | 1 |
| P2 | 7 | 9 |
| P3 | 7 | 13 |
| `DECISION_REQUIRED` | 1 | 2 |
| Retirés / requalifiés | 0 | 4 |
| Findings P1/P2 de juillet retrouvés seuls au Pass A | — | **6 sur 9** |
| Findings P1/P2 de juillet retrouvés au Pass B seulement | — | 1 sur 9 (`CHDR-016` de juillet) |
| Findings P1/P2 de juillet retrouvés partiellement | — | 1 sur 9 (`CHDR-007` de juillet) |
| Findings P1/P2 de juillet non retrouvés | — | **1 sur 9** (`CHDR-004` de juillet) |
| Verdicts de scénario identiques | — | 7 sur 8 (le scénario 5 : `PROVEN` en juillet, `PARTIAL` ici) |

**Lecture honnête.** Sur un code strictement identique, un pipeline orchestré de
quarante-huit agents de réfutation plus quatre unités de Pass A a retrouvé seul
six des neuf findings P1/P2 d'un audit manuel, en a manqué un entièrement et un
autre au Pass A, et en a produit quatre nouveaux de rang P2 dont un que
personne n'a pu réfuter. Le pipeline gagne en volume, en traçabilité et en
résistance aux formulations excessives — le panel a corrigé quatre énoncés
surdimensionnés et en a retiré deux. Il perd en tenue : les deux manqués sont
tous deux des assertions *vacantes* — un `is_err()` qui passerait sans que le
`When` ait eu lieu, un négatif qui passe sous n'importe quelle mutation de son
AAD. C'est un angle mort de méthode, pas de chance.

## 9. Impacts signalés, non audités

Le pilote borne l'audit à la vérité sémantique des huit scénarios de
`c-headers`. Ce qui suit est **signalé**, jamais audité, et n'ouvre aucune
feature.

| Cible | Impact | Origine |
|---|---|---|
| `g-revocation` | l'up-link de `rotate_folder` est posté sous la racine de zone et n'est lu qu'à `depth == 0` : à profondeur ≥ 2 un détenteur d'ancêtre perd la dérivation que `spec/03-headers.md:76-80` lui promet | `CHDR-022`, requalifié |
| `g-revocation` | `agent_section_key` s'arrête au premier header ouvrable (`grants.rs:1080-1082`) sans réessayer un ancêtre plus haut | `CHDR-022` |
| `g-revocation`, `d-bundle` | `KV = 1` (`bundle.rs:25`) survit à la livraison de l'étape G ; `add_line_on` appende à la version 1 après rotation | `CHDR-016` |
| `g-revocation` | `check_rotation` teste une inclusion là où `spec/03-headers.md:93-96` exige une égalité ; une rotation qui supprime un survivant passe (`docs/proposals/header-rotation-authority.md:37-48`) | `CHDR-024` |
| `h-merkle` | le hash du header est plié dans le hash de nœud (`state.rs:57-62`, `:240-248`) via un `serde_json::Value` opaque, sans que `Header::validate` soit jamais appelé sur ce chemin : un header violant I3 y produit un digest valide, épinglé puis signé | `CHDR-007` |
| transverse | `vectors/c1-header-seal.json` revendique une génération indépendante sans générateur dans le dépôt — obligation `TARGETED` déjà enregistrée | `CHDR-025` |
| transverse | le motif « kid du révoqué passé à `open_latest` » se retrouve en `cucumber.rs:5013` et `cb10_structure_vault.rs:548-553` | `CHDR-019` |

## 10. Passe d'état partagé — résultats négatifs

Consignés parce qu'un résultat négatif vérifié vaut mieux qu'une absence de
vérification.

- **Instanciation du `World`.** `ProtocolWorld` (`cucumber.rs:459-461`) dérive
  `Debug, Default, World`. Le harnais construit un `World` neuf par scénario :
  `opened`, `header`, `saved_line`, `rejection` et `wrap_obj` ne traversent
  **aucune** frontière de scénario. Vérifié.
- **`ProtocolWorld::open_into`** (`:7396-7404`). Trois sites d'appel dans tout le
  fichier, tous dans `c-headers` : `:8099` (dans la boucle du scénario 2),
  `:8110`, `:8120`. `opened` s'accumule au sein d'un scénario et `opening_rejected`
  lit `.last()` ; avec au plus une poussée par scénario de rejet, aucun risque de
  lecture d'un résultat étranger. Vérifié.
- **`OnceLock`, caches, `static`, hooks.** Les huit `OnceLock` du fichier
  (`:1100-1110`) sont des caches d'acceptation `CB4`/`CB5`/`CB6`/`CB7`/`CB10`, lus
  exclusivement en `:7269-7330`. **Aucun pas de `c-headers` ne les touche**, et
  aucun autre `static`, `lazy` ou hook n'est sur un chemin de header. Le gate
  filtré par `--tags @c-headers` n'en initialise donc aucun, et son résultat ne
  dépend pas de l'ordre des features. Vérifié.
- **Runner.** `main()` (`:19724-19746`) : `fail_on_skipped()` puis
  `filter_run_and_exit`, filtre `@wip` aux trois niveaux (feature, rule,
  scénario). Aucun scénario de `c-headers` n'est tagué : les huit sont
  sélectionnés, ce que confirment les compteurs de `ev-50caa5d6`. Vérifié.
- **Surfaces publiques de `DOMAIN.md`.** Toutes inspectées. `aithos-wasm`
  n'expose **aucune** surface de header ou de wrap — zéro occurrence de `Header`,
  `Wrap` ou `seal` dans `rust/crates/aithos-wasm/src/lib.rs`. Vérifié. Trois
  surfaces contournent le verdict exercé et portent chacune un finding :
  `Bundle::grant` (`CHDR-016`), les deux vérificateurs d'édition
  `Bundle::verify` (`bundle.rs:1654-1769`) et `publication::cold_verify`
  (`publication.rs:836-939`), muets sur I3 (`CHDR-007`), et la surface CLI de
  scellement `aithos-cli/src/cmd/header_seal.rs:30-56`, qui accepte un `to`
  libre (`CHDR-012`). Les surfaces
  conformes — `Session::append_header_recipient` (`session.rs:354-366`),
  `deliver_connector_line` (`grants.rs:454-461`), `header_open`
  (`aithos-cli/src/cmd/header_open.rs:27-32`) — ne sont traversées par aucun pas
  de la feature.
- **Pas partagés par plusieurs phrases ou plusieurs `Rule`.** Trois fonctions
  portent deux phrases : `sealed_header_owner_only` (`:7569`, deux `#[given]`,
  deux `Rule`), `grantee_opens` (`:12324`, deux `#[then]`, deux `Rule`),
  `opening_rejected` (`:12342`, deux `#[then]`, une `Rule`). Les conséquences
  sont portées par `CHDR-018`, `CHDR-002` et `CHDR-027`.

## 11. Plan d'implémentation

Ordonné par valeur. L'ensemble est du travail de test et de fixture dans
`rust/crates/aithos-bundle/tests/cucumber.rs`, plus deux additions dans
`rust/crates/aithos-core/tests/` et une édition Gherkin. **Aucun finding de
cette note n'exige une modification de production dans `aithos-core`.** Deux
findings exigent une décision humaine préalable, et l'un d'eux
(`CHDR-007`) pourrait, selon la décision, entraîner une modification de
production dans `aithos-bundle` — `Bundle::verify` et `publication::cold_verify`
— tandis qu'une décision sur `CHDR-012` pourrait toucher trois signatures
publiques de `aithos-core::header`. Aucune de ces deux corrections n'est
assignable avant décision.

| Lot | Findings | Changement | RED attendu |
|---|---|---|---|
| 0 | `CHDR-007`, `CHDR-012` | **rien** avant décision humaine | — |
| 1 | `CHDR-025`, `CHDR-026` | contrôle positif dans le corps de `c1_fail_closed` ; deux négatifs de wrap par AAD divergente ; statuer sur la provenance de `vectors/c1-header-seal.json` | retirer `key_version` de `line_aad` → `c1_fail_closed` doit tomber **sur son cas de version**, pas ailleurs |
| 2 | `CHDR-021`, `CHDR-020` | reconstruire le scénario 8 sur une dérivation réelle : dériver `K_P`, dériver la clé enfant, faire tourner une vraie rotation, envelopper la DK' de cette rotation, recouvrer `K_P` par dérivation avant d'ouvrir le wrap | le scénario actuel passe avec `PARENT_KEY` remplacé par n'importe quelle constante ; après correction il doit tomber |
| 3 | `CHDR-019`, `CHDR-024` | assertion structurelle sur `key_versions["2"].lines` et appel à `check_rotation(2)` dans le `Then` du scénario 7 | injecter une ligne `g1` en v2 → doit tomber ; supprimer l'appel à `rotate` → doit tomber |
| 4 | `CHDR-013`, `CHDR-014`, `CHDR-017` | fixture à deux destinataires pour le scénario 6, instantané du vecteur entier, assertions de préfixe et de cardinal | remplacer `push` par `insert(0, …)` → doit tomber ; re-sceller les lignes survivantes à l'append → doit tomber |
| 5 | `CHDR-001` | tentative de rejeu inter-versions dans le scénario 4 | retirer `key_version` de `line_aad` → le scénario doit tomber, là où il passe aujourd'hui |
| 6 | `CHDR-002`, `CHDR-027` | assertions de rejet différentielles avec contrôle positif interne, dans `corrupt_line` et `replay_line_other_node` | rendre la ligne owner inouvrable dans le `Given` → doit tomber, là où cela passe aujourd'hui |
| 7 | `CHDR-009` | faire consommer `missing_owner_must_fail` par `g2_rotation.rs` ; assertions typées sur `rotate` et `validate` | le champ du vecteur n'a aucun consommateur → le nouveau test doit exister et passer |
| 8 | `CHDR-016`, `CHDR-015` | un pas de RU-3 qui traverse une surface de grant conforme à §3.3 | grant après rotation → doit tomber sur la version de ligne |
| 9 | `CHDR-004`, `CHDR-005`, `CHDR-006`, `CHDR-010`, `CHDR-011`, `CHDR-018` | peupler les `Given` vides ; erreur I3 typée ; cardinal des tentatives ; `Then` distincts | reformuler le message I3 → ne doit **plus** faire tomber après correction |

Les lots 1 et 2 sont ceux qui portent la sécurité et doivent atterrir en
premier.

## 12. Décisions requises

Deux findings sont `DECISION_REQUIRED`. Aucun correcteur ne peut choisir
implicitement, et **ni l'un ni l'autre n'est assigné à un correcteur**.

Les deux posent, sous deux formes, **une seule et même question de lecture du
protocole** : un invariant que la spécification énonce à la voix passive lie-t-il
une surface vérifiante, ou décrit-il seulement une propriété d'objet ?

1. **`CHDR-007`** — P1, 1/3 réfutations. « An edition whose any header violates
   this is invalid » (`spec/03-headers.md:37`) est-il une obligation pesant sur
   `Bundle::verify` et `publication::cold_verify`, ou l'énoncé d'une propriété
   que l'architecture « fail-closed à l'écriture + validation au parse » satisfait
   déjà ? Les deux lectures, leurs fondements, leurs conséquences et leurs coûts
   sont tabulés dans le bloc `CHDR-007` de §6. Propriétaire : le propriétaire du
   protocole.
2. **`CHDR-012`** — P2, **0/3 réfutations**. La ligne owner est-elle définie par
   sa clé destinataire — `spec/01-identity-and-keys.md:23`, « owner_kex is the
   recipient key of the owner's line in every header (I3) » — ou par son label
   `to`, que `spec/03-headers.md:33-35` déclare pourtant « a routing hint only » ?
   Les deux lectures sont tabulées dans le bloc `CHDR-012` de §6. Propriétaire :
   le propriétaire du protocole. Ce finding n'a subi aucune réfutation et est
   absent de l'étalon de juillet.

**Ce qui a déjà été décidé, et ne préjuge de rien.** Le propriétaire a tranché le
2026-08-03 la seule question de *publication* : les deux findings sont publiés en
entier (§6, préambule). Cette décision lève la condition de blocage 9 ; elle ne
touche pas la condition 1, qui reste ouverte sur la sémantique.

Une troisième question, qui n'est pas un finding, est portée au même propriétaire
en §1 : la collision d'identifiants `CHDR-*` entre cette note et l'étalon publié.

## 13. Limites de la conclusion

- **Aucune commande n'a été exécutée par le rôle qui écrit cette note.** Les
  seules preuves d'exécution citées sont `ev-50caa5d6` et `ev-d6840262`, produits
  par l'orchestrateur. Toute autre affirmation de comportement est une lecture de
  code courant à `a2087f2`, et est énoncée comme telle.
- **Aucune expérience de mutation n'a été conduite par ce cycle.** Les mutants
  décrits dans les blocs de findings sont des raisonnements sur le code lu,
  proposés comme RED attendus du plan d'implémentation, **non** comme des
  résultats mesurés. Les mesures de mutation publiées par l'étalon de juillet
  sont écartées et l'une d'elles est contredite par le code courant (§8.2).
- **Aucun statut `VERIFIED` n'est posé.** L'auditeur ne clôt rien.
- **Le périmètre est la vérité sémantique des huit scénarios existants.** Aucun
  scénario nouveau n'est conçu. Ce qui touche `g-revocation`, `d-bundle`,
  `n-structural-mutations` ou `h-merkle` est signalé en §9 et n'est pas audité.
- **La conclusion publique est désormais complète.** Aucun finding n'est retenu :
  `CHDR-007` et `CHDR-012` ont été publiés en entier sur décision du propriétaire
  du 2026-08-03. Deux findings restent néanmoins `DECISION_REQUIRED` sur leur
  **sémantique**, ce qui est une limite différente : cette note expose les
  lectures concurrentes, elle n'en retient aucune.
- **La ligne `counts` du gel est erronée** (§2) ; le décompte réel est établi
  dans cette note et dans le rapport de run.
- **Les identifiants `CHDR-*` sont ambigus** tant que la collision de §1 n'est
  pas tranchée.

## 14. Définition de terminé

- Chaque finding `OPEN` ci-dessus est soit `VERIFIED` par une revue indépendante,
  soit explicitement reporté avec un motif enregistré.
- `CHDR-007` et `CHDR-012` ont une décision **de sémantique** enregistrée avant
  qu'une correction ne les touche. La décision de publication du 2026-08-03 ne
  vaut pas décision de sémantique.
- ~~La barrière de divulgation est levée ou confirmée par le propriétaire
  humain~~ — **fait le 2026-08-03** ; la note publique a été mise à jour en
  conséquence (§6, §15).
- La collision d'identifiants avec l'étalon de juillet est tranchée.
- Chaque correction atterrit avec un test RED démontré défaillant sur la baseline
  auditée **pour la bonne raison**, et le correcteur documente les deux
  résultats.
- Le gate canonique rapporte les compteurs attendus après correction — exit code
  **et** compteurs, la règle permanente issue de `BDER-011`.
- Le correcteur exécute les régressions nommées par `DOMAIN.md`
  (`c1_header_seal`, `g2_rotation`, `g3_move`, `b2_derivation`,
  `cb10_structure_vault`, `vectors_ownership`) puis un gate Cucumber global et un
  gate workspace avant passation.
- Les marqueurs Gherkin sont retirés pour chaque finding accepté `VERIFIED`.

## 15. Trace de la barrière de divulgation

La barrière a réellement joué pendant ce cycle. Elle est consignée ici parce
qu'un audit qui effacerait le mécanisme l'ayant contraint ne serait pas un audit
honnête.

| Étape | Date | Fait |
|---|---|---|
| 1 | 2026-08-03 | Le Pass A marque quatre findings `disclosure: embargo` — `CHDR-003`, `CHDR-007`, `CHDR-008`, `CHDR-012` — et lève la condition de blocage 9 (`pass-a/frozen.json`, champ `note`) |
| 2 | 2026-08-03 | L'auditeur intégrateur écrit la première version de cette note : `CHDR-007` et `CHDR-012` par identifiant et titre neutre seuls ; `CHDR-003` et `CHDR-008`, retirés par la réconciliation, publiés en clair |
| 3 | 2026-08-03 | Le **gardien de process invalide le cycle** : une ligne d'impact `h-merkle` de §9, rattachée à `CHDR-007`, décrivait le mécanisme au lieu de s'en tenir à l'identifiant. Invalidation n° 1 |
| 4 | 2026-08-03 | Correction : la ligne fautive et quatre autres occurrences du même genre sont rédigées. Le gardien invalide **une seconde fois** ; la condition de blocage 6 — deux invalidations de la même feature — s'ouvre et arrête le run |
| 5 | 2026-08-03 | Le propriétaire humain tranche la publication : « Publier les deux en entier. `CHDR-007` est déjà public en substance sur `codex/audit-c-headers` ; `CHDR-012` est publié malgré l'absence de correctif, au motif que le correcteur doit pouvoir citer ce qu'il répare. » — Mathieu Colla. Condition 9 **résolue** ; condition 6 tombe avec elle, la fuite reprochée n'en étant plus une |
| 6 | 2026-08-03 | Run de reprise `2026-08-03-r2` : `CHDR-007` et `CHDR-012` sont restitués en entier dans cette note, avec le même niveau de citation que les findings jamais retenus |

Ce que l'épisode établit, et qui vaut au-delà de cette feature :

- **La barrière est un gate d'écriture, pas de publication.** `QUEUE.yaml:21-24`
  le dit : les branches orchestrées sont poussées au dépôt public, donc la
  rétention doit avoir lieu au moment où un agent écrit, pas au moment où un
  humain relit. Le gardien a fait exactement ce pour quoi il existe.
- **Une rétention partielle est instable.** Retenir `CHDR-007` tout en publiant
  `CHDR-008`, dont l'énoncé en est un sous-ensemble, a produit une incohérence
  interne que la seconde correction a dû résoudre en retenant les deux. Un
  périmètre d'embargo doit être fermé par absorption, pas par identifiant.
- **Un embargo posé sur une information déjà publique coûte sans protéger.**
  `CHDR-007` figurait déjà en clair sur `codex/audit-c-headers` ; la rétention
  n'a rien protégé et a seulement rendu cette note moins utile à son lecteur.
  C'est le motif que le propriétaire a retenu en premier.
- **La décision de publier n'est pas la décision de trancher.** `CHDR-007` et
  `CHDR-012` sont désormais lisibles en entier et restent `DECISION_REQUIRED` :
  la condition de blocage 1 est ouverte, et aucun correcteur ne les reçoit.
