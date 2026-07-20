# Note à la piste provider — gate temporaire core/bundle

**Date :** 2026-07-18

**Destinataire :** agent en cours de construction du provider cloud Aithos

**Décision Mathieu :** le développement provider peut être mis en pause autant que
nécessaire jusqu'à la fermeture complète du protocole dans `aithos-core` et
`aithos-bundle`.

**Documents de référence :**

- `docs/HANDOFF-CORE-PROTOCOL-COMPLETE-2026-07-18.md`
- `docs/HANDOFF-CORE-PROTOCOL-LOT1-CONTRACTS-2026-07-18.md`
- `docs/HANDOFF-CORE-BUNDLE-PROTOCOL-ACTION-PLAN-2026-07-18.md`

---

## Message court

Le provider peut continuer à industrialiser le **transport, le stockage opaque et
l'infrastructure**, mais il doit rester fail-closed sur toute décision d'autorité
dont le contrat dépend encore de `aithos-core`/`aithos-bundle`.

L'état transitoire actuel est une barrière correcte :

- les routes manifest/certificats/Gamma non implémentées répondent encore
  `501 not_implemented` dans
  `rust/crates/aithos-provider/src/service.rs:257` ;
- une identité mandatée n'est pas acceptée par le squelette d'enveloppe actuel ;
- aucune de ces limites ne doit être remplacée par une réimplémentation locale des
  règles core.

Le provider ne doit jamais décider seul si un mandat, un changeset, une contrainte ou
une mutation est autorisé.

---

## Ce qui peut continuer

Ces travaux ne figent pas le protocole de publication :

- backend durable générique derrière le stockage d'objets :
  get/put, streaming, limites, checksums, idempotence et collecte d'orphelins ;
- primitive CAS générique sur un tuple opaque `attendu → nouveau`, sans fixer encore
  les champs, heads ou erreurs métier Aithos ;
- nonces, anti-replay de transport, horloge injectée, authentification de
  l'enveloppe P1 déjà vectorisée ;
- redaction, discipline de logs, health, métriques et durcissement HTTP ;
- harnais E2E lançant le vrai binaire par HTTP avec backend durable, arrêt/restart
  et nouveau client, sans prétendre encore valider une publication Aithos ;
- S3/Dynamo/IAM, CI, observabilité, tests de panne et exploitation ;
- tunnel/relay, TLS/SNI, multiplexage et control plane ;
- witness/checkpoints et détection d'équivoque déjà vectorisés, sans brancher le
  witness sur un feed de publications dont le wire n'est pas figé.

Ces travaux doivent rester indépendants des clés de contenu et du plaintext client.

---

## Ce qui est gelé

Attendre les contrats, vecteurs et API core/bundle avant de développer :

- acceptation d'une requête mandatée ;
- toute copie de `verify_chain`, `covers`, révocation, contraintes, compteurs,
  wildcard ou classe d'action ;
- PUT manifest/certificats/Gamma et endpoint final de publication ;
- batch/heads/sync lorsqu'ils engagent le wire ou l'autorité de publication ;
- mapping final des erreurs protocolaires ;
- tuple CAS protocolaire
  `(manifest head, gamma head, height, parent)` et transaction de visibilité ;
- changeset et édition déléguée normale ;
- authorship déléguée de `public` ;
- preuves keyless de mutations `self` ;
- catalogues `read/act/binding` et règle wildcard hors binding ;
- layout final du vault `/x/<connector>`, `.config`, headers/lignes et rotation ;
- alimentation du witness depuis le head canonique ;
- `RemoteStore`, SDK réseau ou bibliothèque cliente réutilisable dans ce dépôt.

Le harnais HTTP des tests provider n'est pas le futur SDK.

---

## Artefacts attendus de `aithos-core`

La reprise provider dépendra d'une API pure équivalente à :

```text
verify_publication(
    artefacts_publics,
    head_canonique_actuel,
    temps_injecté,
    état_de_révocation,
) -> verdict typé
```

Le nom Rust et le wire ne sont pas encore figés. La propriété attendue est :

- zéro I/O, réseau, horloge ou RNG implicite ;
- vérification de forme/version, signatures, DID, preuve de possession ;
- chaîne, atténuation, révocation, freshness et périmètre ;
- contraintes tier V et receipts/attestations publiques exigées pour tier X ;
- changeset, authorship, Gamma, roots, parent/height et anti-replay ;
- erreurs protocolaires typées, sans fuite de donnée scellée ;
- aucune clé de contenu ou donnée client en clair.

Le provider ne recopiera aucune branche de cette API. La répartition visée est :

- `aithos-core` porte ce verdict sémantique pur sur des artefacts déjà typés ;
- `aithos-bundle` décode le paquet, vérifie layout/hashes/atteignabilité, puis
  délègue le verdict au core ;
- après validation du micro-gate G-D, le provider appelle cette façade keyless
  Bundle unique puis ne réalise que stockage opaque, transport et CAS.

Ne brancher ni une API Core directe ni une façade Bundle avant validation de G-D.

---

## Artefacts attendus d'`aithos-bundle`

Le bundle devra fournir, sans réseau :

- un plan de publication sérialisable ;
- les objets opaques/content-addressed à transférer ;
- les artefacts et preuves publics nécessaires au verifier ;
- le changeset typé et le delta Gamma ;
- le tuple CAS attendu/nouveau ;
- l'ingestion d'un jeu d'artefacts téléchargés dans un store local vierge ;
- une vérification froide owner/grantee depuis ces seuls artefacts.

Le bundle ne doit pas embarquer HTTP, DNS, retry ou authentification provider.

---

## Conditions de reprise de la publication provider

Ne reprendre l'intégration protocolaire que lorsque :

1. CB13 Core + Bundle est entièrement vert, sa matrice est complète et aucun
   `@wip` pertinent Core + Bundle ne subsiste ;
2. les contrats Gherkin core/bundle sont validés et committés ;
3. le wire concerné est figé par un oracle et des vecteurs indépendants ;
4. le verifier pur core est vert contre ces vecteurs ;
5. le bundle produit la même enveloppe et prouve son atomicité ;
6. un store local vierge peut ingérer les artefacts puis vérifier l'édition à froid ;
7. les erreurs publiques et invariants CAS sont stabilisés ;
8. l'ownership des fichiers provider est explicitement attribué.

La reprise se fera alors dans cet ordre :

1. adapter le service à la façade keyless Bundle validée, qui délègue au verifier
   Core ;
2. mapper les verdicts typés sans logique parallèle ;
3. rendre le commit CAS atomique sur le backend durable ;
4. brancher witness et heads canoniques ;
5. exécuter le vrai E2E :

```text
bundle grantee
→ HTTP provider
→ arrêt/restart
→ téléchargement dans un nouveau store local
→ cold verify
→ lectures owner/grantee
```

Aucun mock du protocole, aucune clé de contenu et aucun plaintext.

---

## Ownership immédiat

Dans le worktree actuel :

- `rust/crates/aithos-provider/**` est entièrement non suivi et appartient à la
  piste provider ;
- `rust/Cargo.toml`, `rust/Cargo.lock`, les vecteurs P et les documents provider
  portent aussi ses travaux en cours ;
- la session core/bundle ne les modifie, ne les stage et ne les commit pas.

Les futurs contrats :

- `rust/crates/aithos-provider/tests/features/store/store-publication.feature`
- `rust/crates/aithos-provider/tests/features/store/store-cold-roundtrip.feature`

doivent être attribués à une seule piste avant création. Ne pas les créer en
parallèle dans le même worktree.

Si la piste provider atteint l'un des points gelés avant le gate core/bundle, elle
prépare une note d'interface ou un test de harnais non protocolaire, puis s'arrête
sans inventer le contrat manquant.
