# Prompt de reprise — Aithos Core, Lot 1 contrats

Copier le bloc ci-dessous dans une nouvelle tâche Codex.

```text
Tu reprends uniquement le Lot 1 — contrats de complétude — dans :

/Volumes/Math17/aithos/v2/code/aithos-core

Ta source de reprise principale est :

docs/HANDOFF-CORE-PROTOCOL-LOT1-CONTRACTS-2026-07-18.md

Lis-la entièrement avant toute action. Lis ensuite entièrement sa source parente :

docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md

Puis lis, dans l'ordre imposé par ces handoffs, README, specs 00–10, execution
plan, handoffs mandats/provider/gateway, features pertinentes, rituels
BDD/vectors-first/pure-core et code réel des crates core, bundle, provider,
gateway, CLI et WASM.

Le Lot 0 est terminé. Mathieu a explicitement accepté les recommandations et demandé
de les acter. D1–D9 et les décisions transverses T1–T3 du handoff Lot 1 sont
opposables. Ne rouvre pas leurs choix déjà nets. La relation `.config`/classe D8 est
le seul sous-gate explicitement non ratifié. Ne refais pas la baseline exhaustive,
mais vérifie l'état courant avant de toucher un fichier.

Décisions à appliquer sans réinterprétation :

- D1 : id=<sid> est exact ; whole-zone ou même id seulement le couvre ; dir/tag ne
  couvre jamais id.
- D2 : delete implique read ; aucune suppression aveugle.
- D3 : édition déléguée v1 normale, signée par le grantee feuille, à acteur et
  chaîne uniques ; plusieurs chaînes produisent des éditions séparées ; changeset,
  Gamma, roots, certificats et CAS sont liés et keyless-verifiables.
- D4 : public distingue authorship owner et grantee ; un grantee ne signe jamais
  comme owner.
- D5 : self reste opaque ; edit/delete par SID, création par zone append/write ou
  SID préalloué ; proofs/commitments avant-après sans structure en clair.
- D6 : sections et dossiers suivent read/edit/append/delete/write ; move exige
  source + destination et les rewraps nécessaires ; trust-root/recovery owner-only ;
  issue/revoke/config restent des droits dédiés.
- D7 : toute mutation déléguée passe par un verdict pur commun avant effet et au
  rejeu froid ; max_actions reste réservé aux actions, avec un compteur/limite
  mutations et un compteur/limite total explicites, à nommer seulement après le
  contrat Gherkin et avant leur wire.
- D8 : classe canonique read|act|binding prouvée par manifeste signé et
  owner-approuvé ; la chaîne pince le catalogue/version autorisé ; wildcard couvre
  read/act seulement ; binding exact + reçu owner co_sign ; un mapping legacy
  write→act n'existe que dans une migration explicite, ne vaut jamais binding et
  impose le réenrôlement.
- D9 : vault isolé /x/<connector> ; seul act.x.<connector>.config, droit réservé
  exact jamais couvert par wildcard, ouvre la config de ce connecteur ; un simple
  act ne livre jamais le credential ; tool-host, grantee, rotations et révocations
  restent isolés. Ne lui attribue pas automatiquement une classe D8 ni co_sign ; si
  sa relation au catalogue doit être normée, STOP et demande une validation ciblée.
  L'accès exige chaîne exacte ET ligne /x/<connector>, ni l'une ni l'autre ne suffit
  seule.
- T1 : max_children compte les enfants directs et n'est pas supprimable.
- T2 : contrainte racine connue toujours validée ; inconnue tolérée sur chaîne
  feuille mais toute sous-délégation échoue faute de loi d'atténuation.
- T3 : depth=0 et sélecteurs dupliqués invalides ; forme complète du mandat,
  révocation et consumption appartiennent au verdict froid.

MISSION OPPOSABLE :

Écris le contrat de complétude et rien d'autre :

1. redline uniquement les contradictions de spec ratifiées ;
2. ajoute les scénarios Gherkin manquants, tous @wip ;
3. couvre id/lattice, mutations public/circle/self, atomicité, édition normale
   grantee, authorship public, self keyless, opérations structurelles, contraintes,
   classes connecteurs, vault isolé, provider CAS et cold roundtrip ;
4. présente le diff complet à Mathieu ;
5. STOP avant commit jusqu'à sa validation explicite.

Le Lot 1 n'autorise PAS :

- une implémentation Rust, un step Cucumber ou un helper ;
- un vecteur, oracle ou fixture cryptographique ;
- un champ JSON, algorithme, hash, signature, version ou migration wire ;
- un changement Cargo, CLI, WASM, gateway runtime, provider runtime ou client ;
- le retrait d'un @wip ou l'affaiblissement d'un scénario vert ;
- un push, merge, déploiement, changement de branche, clean, reset ou restore.

Le dépôt est sale et les travaux existants appartiennent à Mathieu. Après les
lectures obligatoires, relève :

git status --short --branch --untracked-files=all

Compare chaque cible au relevé du handoff. Stage uniquement des fichiers nommés de
ta tranche, et seulement après validation du contrat. Si une spec ou feature cible
est déjà modifiée, STOP.

ATTENTION PROVIDER :

rust/crates/aithos-provider/** est actuellement entièrement non suivi et appartient
à la piste P. Sans autorisation explicite de Mathieu pour ces deux chemins, STOP
avant toute écriture du Lot 1 :

- rust/crates/aithos-provider/tests/features/store/store-publication.feature
- rust/crates/aithos-provider/tests/features/store/store-cold-roundtrip.feature

Ne contourne pas cet ownership en déplaçant le contrat provider dans une feature
bundle. Si l'autorisation n'est pas déjà ajoutée au message de lancement, présente
les deux fichiers proposés et demande-la avant de créer ou modifier le moindre
contrat. Une scission L1a/L1b exige une décision explicite, deux gates et deux
commits ; L1a seul n'est jamais « Lot 1 complet ».

RÉPARTITION ATTENDUE :

- ajouter uniquement des scénarios @wip à features/d-bundle.feature pour la parité
  owner public/circle/self, publication, Gamma et rollback atomique ;
- étendre features/e-mandates.feature pour D2/T3 ;
- étendre features/e-mandate-sections.feature pour D1/id ;
- étendre features/f-plus-constraints.feature ;
- ajouter uniquement des scénarios @wip à features/g-revocation.feature ;
- étendre features/l-delegated-writes.feature ;
- créer features/m-delegated-editions.feature pour l'édition et sa vérification
  offline depuis un store local reconstruit ;
- créer features/n-structural-mutations.feature ;
- créer features/o-connector-classes-vault.feature ;
- compléter features/i-concurrency.feature pour fork/merge/résolution après conflit,
  jamais pour simuler le CAS provider ;
- ajouter des scénarios @wip à
  rust/crates/aithos-gateway/tests/features/gateway-mandates.feature, sans runtime ;
- après autorisation seulement, créer les deux features provider nommées ci-dessus.

store-publication.feature possède expected-head/CAS et les refus keyless.
store-cold-roundtrip.feature exige vrai binaire service, HTTP, backend durable,
restart, puis nouveau process client et store vide.

features/k-integration.feature reste explicitement offline. Un clone de MemStore,
MemObjects ou un mock du protocole n'est jamais un cold roundtrip provider.

Chaque Feature touchée nomme les décisions D#/T# qu'elle matérialise. Chaque scénario
est testable et distingue préconditions, effet, preuve, et absence d'effet en cas de
refus. Le Gherkin décrit des propriétés observables, pas des champs wire inventés.

PROPRIÉTÉS CONTRACTUELLES OBLIGATOIRES :

- un grantee agit avec clé privée ET chaîne valide ; l'owner n'est ni acteur ni
  signataire de l'édition, sauf receipt/co-sign explicitement exigé par une
  obligation ;
- possession d'une clé de contenu seule toujours refusée ;
- create/edit/delete/write délégués sur public/circle/self ;
- état + blobs/index/headers/wraps + Gamma atomiques ;
- bundle local après tout refus/panne = byte-for-byte identique ; une éventuelle
  télémétrie de refus reste hors bundle et hors preuve Gamma ;
- provider après refus/conflit = manifest head, gamma head, height/parent inchangés
  et aucune édition partielle joignable ; seuls des blobs opaques non référencés et
  collectables peuvent subsister ;
- édition grantee normale à chaîne unique, chaque changement couvert ;
- signature public owner/grantee distincte ;
- self keyless sans nom, chemin, tag, contenu ou structure ;
- contraintes tier V et révocations rejouées au cold verify ; pour tier X, le
  provider vérifie receipts/attestations publiques sans demander le plaintext ;
- wildcard ne couvre jamais binding ;
- .config exact et connector-scoped, soumis aux contraintes/obligations du mandat ;
- simple act sans ligne vault ni credential côté grantee ;
- un grantee .config n'ouvre le credential qu'avec sa ligne exacte ; le provider ne
  l'ouvre jamais ;
- tool-host agit seulement après verdict + log-before-effect ;
- CAS concurrent : un gagnant, un conflit stable, aucun état partiel ;
- provider sans clé/plaintext, y compris logs et erreurs ;
- enveloppe transport et autorisation publication indépendantes : si l'une est
  invalide, la requête est refusée même lorsque l'autre est valide ;
- vrai binaire service + HTTP + backend durable conservé après restart + nouveau
  process client + store vierge + cold verify owner/grantee, sans état injecté par
  le harnais.

LIVRABLES AVANT LE GATE HUMAIN :

- tableau des redlines spec avec leur décision source ;
- liste exacte et diff complet des specs/features seulement ;
- inventaire scénarios et @wip avant/après ;
- matrice Lot 0 mise à jour dans le rapport de revue avec
  Gherkin = contractuel @wip, jamais complet ; elle n'entre pas dans le commit ;
- preuve que zéro code, step, vecteur, wire ou fichier sale de Mathieu a changé.

Présente ces livrables à Mathieu puis STOP. Ne commite jamais sur simple absence de
réponse.

APRÈS VALIDATION EXPLICITE DU DIFF :

- rejoue les validations syntaxiques et la baseline pertinente avec target isolé ;
- vérifie que tous les scénarios verts et leurs compteurs restent verts ;
- stage chaque fichier par son nom exact ;
- inspecte le diff staged et exclue Cargo, provider existant, vecteurs et scories,
  sauf les deux nouveaux fichiers provider explicitement autorisés ;
- crée un commit de contrat isolé :
  test(protocol): add D1-D9 completeness contracts @wip
- prépare un handoff LOT1-CONTRACTS-DONE avec hash, fichiers et compteurs ;
- STOP sans commencer le Lot 2.

Une capacité reste incomplète à l'issue du Lot 1 : @wip signifie contrat seulement.
Le cycle suivant sera oracle/vecteur indépendant → test rouge → TDD minimal → retrait
progressif des @wip → vrai E2E → cold verify → fmt/clippy/workspace/WASM → gate.

Aithos-client reste strictement offline et en lecture seule pendant les Lots 1–10.
Le Lot 9 stabilise seulement les API core/bundle qui lui seront destinées. Ses
mutations commencent après le gate protocole Lot 10. Le SDK réseau reste hors de ce
dépôt. Aucun consommateur ne réimplémente une règle du protocole.

Si deux décisions actées se contredisent, si un scénario exige d'inventer une preuve
ou un wire, ou si la règle n'a pas de crate propriétaire clair : STOP et demande une
clarification ciblée sans relitiger les choix déjà nets.
```

Pour autoriser aussi les deux nouveaux fichiers Gherkin dans l'arbre provider,
Mathieu peut ajouter cette phrase au message qui accompagne le prompt :

```text
Je t'autorise explicitement à créer uniquement
rust/crates/aithos-provider/tests/features/store/store-publication.feature et
rust/crates/aithos-provider/tests/features/store/store-cold-roundtrip.feature,
sans modifier aucun autre fichier existant de la piste provider P.
```

Pour fermer aussi le sous-gate `.config` selon la recommandation qui préserve
l'autonomie d'un agent self-hosted, Mathieu peut ajouter :

```text
Je confirme que act.x.<connector>.config est une capacité vault réservée hors du
catalogue des actions D8 : elle reste exact-only et jamais couverte par wildcard,
sans co_sign automatique ; seules les contraintes et obligations explicitement
portées par le mandat s'appliquent.
```
