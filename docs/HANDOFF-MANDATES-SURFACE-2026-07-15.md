# Handoff — surface produit des mandats : plan d'action intégral (P0 → P1)

**Date :** 2026-07-15 (préparé en 9ᵉ session gw, après clôture K+D)
**Branche :** `feat/obligations` (jamais switcher)
**HEAD d'entrée :** `fc86ed1` (docs: Lea demo K+D session close)
**Références, dans l'ordre :** `docs/MANDATES-PRODUCT-GAPS.md` (LE cahier
des écarts — untracked, décision de commit à Mathieu),
`spec/04-mandates.md` / `spec/05-delegation.md` / `spec/08-connectors.md`
(la norme), `docs/GATEWAY-HANDOFF.md` (état express 9ᵉ + protocole
d'environnement §5), `docs/DEMO-LEA-SCENARIO.md` (le gate répétition
générale, INCHANGÉ et prioritaire), puis ce document.
**Mission :** fermer les quatre P0 puis le P1 du cahier des écarts,
BDD-first, en plusieurs sessions, **sans une seule régression** sur
l'existant.

---

## 0. Non-régression : ce que ce chantier change et ne change pas

### La confirmation (vérifiée contre le code le 2026-07-15)

Le chantier est **additif pour l'essentiel**. Le chemin chaud de la démo
Léa (grants lot W, bornes lot P, briefing lot K, beats lot D) n'est
touché par AUCUN lot avant M6 — et M6 le touche par extension serde
compatible, jamais par réécriture. Trois exceptions encadrées, aucune
silencieuse :

1. **M4/P0.1 (`id=`) touche le wire canonique des octets signés.**
   Strictement additif : un mandat SANS `id` doit sérialiser
   byte-for-byte comme aujourd'hui — prouvé par un test
   octets-identiques + un vecteur de conformance AVANT/APRÈS.
2. **M5/P0.2 (atténuation) durcit `verify_chain`.** Par construction ce
   durcissement ne s'applique qu'aux chaînes de longueur > 1 ; le
   gateway n'en produit AUCUNE en production aujourd'hui (toutes les
   chaînes sont `[mandat racine]`). Les seuls scénarios existants à
   chaînes profondes sont côté core (`e-mandates`,
   `l-delegated-writes`) : s'ils cassent, c'est qu'ils reposaient sur
   la laxité actuelle — on tranche explicitement, on n'adapte jamais en
   silence.
3. **M6/P1 (classes `read/act/binding`) touche les manifests scellés.**
   Migration par champ serde additif + re-enrollment (précédent exact :
   le champ `granted` du lot W, absent = défaut historique).

### La baseline figée (2026-07-15, tout vert, à re-vérifier à CHAQUE lot)

| Suite | Compte |
| :-- | :-- |
| aithos-gateway | 62 unit, 4 CLI, **88 scénarios / 473 steps** Cucumber, **6 e2e réseau**, 5 owner-side |
| aithos-core + aithos-bundle + aithos-cli | **97 tests** + Cucumber bundle **203 scénarios / 826 steps** |
| Hygiène | clippy `-D warnings` et `cargo fmt --check` clean (gateway) |

```bash
cd rust && CARGO_INCREMENTAL=0 cargo test -p aithos-gateway            # lots gateway
CARGO_INCREMENTAL=0 cargo test -p aithos-core -p aithos-bundle -p aithos-cli  # lots core
cargo clippy -p <crate touché> --all-targets -- -D warnings
cargo fmt --check -p <crate touché>
```

### Les règles de non-régression (toutes les sessions)

- **Suite complète verte à chaque détag** — la suite du crate touché
  TOUJOURS, la suite workspace (les deux lignes ci-dessus) pour tout lot
  qui touche core/bundle.
- **Les compteurs n'évoluent que par ajout** (nouveaux scénarios
  détaggés, nouveaux tests). Un compteur existant qui bouge sans détag =
  régression, on s'arrête.
- **Jamais modifier un contrat détaggé** (features vertes) sans décision
  Mathieu explicite consignée.
- **Cucumber gateway reste séquentiel** (`max_concurrent_scenarios(1)`).
- Vecteurs de conformance obligatoires pour tout changement du wire
  signé (M4) : figer les octets AVANT de coder.
- Rituel inchangé : décisions → contrats `@wip` committés SEULS → impl
  lot par lot → détag progressif → e2e → docs. Commits par tranche,
  protocole cloud+janitor §5 à la lettre.

---

## 1. Gate M0 — décisions Mathieu AVANT tout code

Aucun contrat ne s'écrit avant ces réponses (elles changent la forme des
scénarios) :

- **(a) Nature des mandats restreints émis par l'interface** : roots
  owner multiples, containment vérifié à l'ÉMISSION contre politique
  Ethos ∩ manifeste (**reco**) — ou sub-mandats d'un « mandat plafond »
  (exige M5 complet d'abord, la délégation récursive restant le chemin
  agent→agent de §05).
- **(b) Cardinalité** : plusieurs mandats actifs par Ethos, mais **un
  seul actif par couple (Ethos, keypair)** (**reco** — zéro choix de
  chaîne au runtime, une interface simple) — ou N par keypair (alors :
  comment le runtime choisit-il la chaîne ? refus d'ambiguïté ?).
- **(c) Clés de contraintes inconnues en sous-délégation** : refus
  fail-closed (**reco**, cohérent avec `deny_unknown_fields` partout) —
  ou copie-identique-exigée.
- **(d) Multi-délégués actifs sur UN même contexte** : le protocole
  encaisse N signataires sur une chaîne, mais le gamma est UNE chaîne et
  le store fs n'a pas de point de sérialisation multi-process. **Reco
  v1** : on émet N mandats (ils dorment, vérifiables offline,
  révocables), mais UN SEUL runner actif par contexte ; le multi-actifs
  simultané attend le `RemoteStore` (§3bis.1). À graver comme limite
  documentée, pas comme surprise.
- **(e) Priorité** : ce chantier passe APRÈS le gate répétition générale
  de la démo Léa (reco), et son ordre relatif à V4 LLM / writes réels
  Ethos / `resources/*` est à fixer.
- **(f) Nommage owner** (reco) : `owner-issue-mandate`,
  `owner-revoke-mandate`, `owner-preview-mandate`.

## 2. Lot M1 — les contrats, committés seuls

- **`tests/features/gateway-mandates.feature`** (gateway) — Rules
  attendues : émission restreinte (sous-ensemble d'outils ⊆ outils
  grantés du manifeste ; bornes du manifeste héritées, non modifiables,
  seulement resserrables par contraintes ; sous-ensemble de
  zones/dossiers — sections quand M4 est là) ; multi-mandats (deux
  délégués simultanés ; chaque acte signé par SA clé et portant SON
  mandat dans `authorized_via` ; révocation ciblée d'un mandat, l'autre
  continue de passer — offline verify à l'appui) ; preview = décision
  (le JSON du preview et le verdict runtime sortent de la même
  fonction) ; refus pédagogiques nommant la raison exacte ; **jamais de
  ligne vault pour un droit `act.*`** (invariant) ; read-model
  (actif/expiré/révoqué, usages restants).
- **Features core** (repo racine) : scénarios `id=`
  (extension d'`e-mandates.feature` ou fichier dédié) ; matrice
  d'atténuation par famille (extension de `f-plus-constraints.feature`).
- Le tout `@wip`, committé SEUL — la sonde habituelle (dé-tagger UN
  scénario) valide que les fichiers sont parsés.

## 3. Lot M2 (P0.4) — la politique effective, fonction pure

```text
droits effectifs = politique Ethos/connecteurs
                ∩ périmètre du mandat (∩ parents)
                ∩ bornes du manifeste (héritées, dures)
                ∩ contraintes applicables
```

- Une fonction pure (proposition : `hub.rs`/`policy.rs`, exposée par
  `core_bridge`) qui prend (manifeste approuvé, chaîne de mandats,
  config, T) et rend le verdict + la DESCRIPTION de la politique
  effective (le read-model embryonnaire).
- **Non-régression par construction** : des tests d'équivalence
  rejouent les verdicts des scénarios grants/bounds EXISTANTS à travers
  la fonction pure et exigent l'égalité avec le runtime. Le chemin
  chaud n'est PAS rebranché dans ce lot (rebranchement éventuel = lot
  ultérieur, après équivalence prouvée sur toute la suite).
- CLI `owner-preview-mandate` (JSON stable pour l'UI, critère
  d'acceptation n°3 du cahier).

## 4. Lot M3 (P0.3) — émettre, lister, révoquer plusieurs mandats

- `core_bridge::owner_issue_mandate(master, label, grantee_pub,
  outils⊆, zones/dirs⊆, contraintes, window)` : validation par la
  fonction M2 (fail-closed : outil non granté au manifeste → refus,
  contrainte élargissante → refus), mint d'un root vers la pubkey
  destinataire, certificat persisté, **grant journalisé**. Registre
  additif des mandats émis (index owner-side, p.ex.
  `gateway/issued.json`) — **le `state.json` du runner ne bouge pas**
  (le runner continue de charger SA chaîne : zéro impact runtime).
- `owner_revoke_mandate(master, label, mandate_id, reason)` :
  `log_revoke_owner` ciblé (mécanique existante, déjà exercée par le
  re-enrollment) + trace au registre.
- Scénario d'attribution : deux clés délégués, deux actes — deux
  signatures distinctes vérifiables offline, `authorized_via` disjoints.
- Invariants testés : la ligne vault `/x/<server>` ne se livre JAMAIS
  sur un `act.*` (custody gateway intacte) ; les 5 tests owner_surface
  existants inchangés (les nouvelles commandes ont les leurs).
- Décision (b) appliquée : refus d'émettre un second mandat actif vers
  une keypair déjà équipée sur cet Ethos (si reco retenue).
- CLI : `owner-issue-mandate`, `owner-revoke-mandate`.

## 5. Lot M4 (P0.1) — le sélecteur `id=` — SESSION CORE DÉDIÉE

- `mandate.rs` : `PerimeterEntry::Ethos` + `id: Option<Sid>` (wire
  additif : None → octets d'aujourd'hui, prouvé) ; parse/serialize du
  sel `id=` ; `covers()` — POINT DE DESIGN à trancher en début de lot :
  `id` ne se compose avec rien (spec §04) — un parent `dir=` couvre-t-il
  un enfant `id=` ? (les sids sont globaux, le containment pur ne peut
  pas résoudre le dossier d'une section) — reco : `id=` n'est couvert
  que par `id=` identique ou par l'entrée de zone entière, documenter.
- `Op` porte le sid de section ; `covers_op` le confronte.
- `grants.rs` : livraison de la header line de LA section au grant ;
  lecture ET écriture par section, `self` compris.
- Vecteur de conformance : un mandat avec `id=` figé octet par octet ;
  un test byte-for-byte sur mandat sans `id`.
- BDD : core + bundle + un scénario gateway (p.ex. grant d'une section
  de briefing précise).
- **Gate de sortie : les DEUX suites complètes** (workspace) + vecteurs.

## 6. Lot M5 (P0.2) — l'atténuation complète — SESSION CORE DÉDIÉE

- `constraints.rs` : validation/normalisation TYPÉE des familles
  connues (caps numériques, budgets, `domains`/allow-lists,
  `action_params`, `heartbeat`, `freshness`, `first_party_only`,
  `counter_sign`, `binding`) ; `constraints_attenuate(parent, child)`
  fail-closed ; clés inconnues → décision (c).
- Branché dans `verify_chain` (donc automatiquement identique offline
  et gateway — même porte).
- Matrice de tests PAR FAMILLE : cap inférieur accepté / cap supérieur
  refusé / allow-list incluse acceptée / suppression d'une contrainte
  héritée refusée / clé inconnue refusée.
- Repasser `e-mandates` et `l-delegated-writes` : toute casse = laxité
  documentée à trancher, jamais un fix silencieux.

## 7. Lot M6 (P1) — binding, classes, read-model

- `covers()` / gateway : un wildcard `act.x.<c>.*` ne couvre JAMAIS une
  action de classe `binding` — design à trancher en début de lot (la
  classe vit au manifeste, connu du gateway ; le core pur ne la connaît
  pas → le contrôle vit probablement gateway-side, ou la classe entre
  dans l'entrée de périmètre).
- Convergence `read/write` → `read/act/binding` : champ serde additif
  sur `ApprovedTool` (précédent `granted`), migration par re-enrollment,
  legacy mono intact.
- Read-model : la fonction M2 enrichie (statut actif/expiré/révoqué,
  contraintes héritées, usages restants via les compteurs gamma, raison
  précise d'un refus) — JSON stable versionné pour l'UI.

## 8. Estimation et séquencement

| Lot | Périmètre | Estimation |
| :-: | :-- | :-: |
| M0 | décisions | 0 (échange) |
| M1 | contrats seuls | ½ session |
| M2 | fonction pure + preview | 1 session gw |
| M3 | émission/révocation multi | 1–1½ session gw |
| M4 | `id=` | 1 session core |
| M5 | atténuation | 1 session core |
| M6 | P1 | 1–1½ session |

M2 → M3 est le chemin critique de l'interface ; M4 et M5 sont
parallélisables en sessions core dédiées ; M6 ferme. Le **gate
répétition générale démo Léa reste prioritaire et indépendant** — ce
chantier n'y touche pas et ne doit jamais le retarder.

## 9. Protocole d'environnement et gates (inchangés)

GATEWAY-HANDOFF §5 à la lettre (profil cloud+janitor : archive → tar →
build/test cloud → retours sha256-croisés fichier par fichier → commits
janitorisés par tranche ; le pont peut flapper : committer tranche par
tranche). Pas de merge `main`, pas de déploiement, aucune donnée réelle.
En fin de chaque session : suites complètes + clippy + fmt, synchro
sha-croisée, état express + §6 GATEWAY-HANDOFF, handoff de reprise.
