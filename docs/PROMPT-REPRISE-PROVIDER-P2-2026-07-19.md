# PROMPT DE REPRISE — Piste P / Provider — P2 (intégration protocolaire de publication)

> À coller dans un contexte frais. Reprend la piste P au point exact du
> 2026-07-19 : **M2 est déployé et validé en production, CB13 (core +
> bundle) est vert, et P2 est débloqué.** Se lit avec
> `code/aithos-core/docs/HANDOFF-PROVIDER-P2-RESUME-2026-07-19.md` (état,
> seams exacts, ordre, interdits), `INFRA-PROVIDER.md` (annexe A normative)
> et `HANDOFF-PROVIDER-AWS.md` (état express).

---

Tu prends la suite de la piste P : le provider Aithos sur AWS, tranche
**P2 — l'intégration protocolaire de publication**. Tu suis le rituel BDD
(features Gherkin AVANT le code, vecteurs indépendants AVANT le code puis
rejeu byte-exact contre le vrai binaire) et tu **STOP à chaque gate** pour
revue humaine (Mathieu).

## DOCTRINE (non négociable)

Le provider déplace des octets et vérifie des **preuves publiques déjà
typées** ; il ne détient jamais de secret client, ne voit jamais de
plaintext, **ne décide jamais**. `covers()` serveur = anti-abus, jamais
l'autorité. Fail-closed partout. Logs expurgés A.8. `aithos-core` et
`aithos-bundle` restent purs (zéro I/O) ; le provider les **consomme et ne
recopie aucune de leurs règles**. Terraform seulement, aucun apply sans
plan lu + parole explicite de Mathieu. Pas de merge `main` sans gate.

## LE CŒUR DE P2 (ce que la NOTE gelait, désormais ouvert)

CB13 vert ⇒ `aithos-bundle` expose une **façade keyless** unique
(`publication.rs`). Ta mission : le store passe de « transport » à
« vérificateur de publication » en **appelant cette façade**, jamais en
réimplémentant le protocole.

- `KeylessPublicationPackage::verify_for_cas() -> Result<VerifiedPublication>`
  — l'entrée provider. `VerifiedPublication { carriers, cas }` avec
  `PublicationCasFacts { subject, new_height, expected_predecessors,
  new_manifest_head, new_gamma_head, roots, gamma_roots, gamma_counts_root,
  reachable_objects, package_digest, mode, resolution_winner, … }`.
- Succès → persister `reachable_objects` (opaque) + **comparer/avancer
  atomiquement les têtes** (CAS A.5). Rejet → variante fermée → un code A.7.
  **Zéro verdict sémantique dérivé de ces champs par le provider.**
- Pour l'**autorisation de requête mandatée** (A.2 #7–#10, le `#9` que P1
  défère en `chain_invalid`) : brancher `aithos_core::operation::
  verify_operation_facts` + `mandate`/`gamma_replay`/`revocation`/
  `constraints`. Le store appelle, ne recopie pas.
- `import_keyless` / `cold_verify` couvrent le store vierge (E2E à froid).

## DÉJÀ FAIT et VERT (ne pas refaire)

- **M2 déployé en prod, gate clos** : store `store.aithos.fr` (td:3,
  `/acme/txt` B.5 live, DNS Route53), relais `relay.aithos.fr` (td:2,
  passthrough SNI aveugle, cert **ACM exportable**, clé jamais dans
  Terraform). Joignabilité HTTPS + relais aveugle **prouvés depuis une
  vraie machine** (register 4 verdicts ; reach 200 servi par le pod ; logs
  = 0 octet applicatif, 0 SNI fantôme).
- **CB13 core/bundle vert** (rejoué : 216 core, 815 scénarios bundle, 447
  workspace, clippy 0, fmt, wasm, 0 `@wip`) — commit `522dfcd`.
- Crate provider P1→P6/M2 committé (`7349cf6`) ; vecteurs `p1..p6` gelés.
- Squelette store P1 en prod refuse **fail-closed** tout ce qui dépasse P1
  (`#9 chain_invalid` mandaté ; `501` sur manifest/certs/gamma/heads/batch/
  sync) — c'est la barrière que tu lèves gate par gate.

## TA MISSION — P2, dans cet ordre (chaque étape = son gate STOP)

1. **Features AVANT le code**, `@wip`, committées seules et **attribuées au
   provider track** (ownership gravé par Mathieu 2026-07-19) :
   `crates/aithos-provider/tests/features/store/store-publication.feature`
   et `…/store-cold-roundtrip.feature`. Redlines A.2–A.5 minimales par gate
   si écart.
2. **Vecteurs p7+ indépendants AVANT le code**, construits **sur les
   paquets keyless exportés par `aithos-bundle`** (jamais de crypto
   réinventée), observés rouges d'abord.
3. **Autorisation mandatée** : brancher `verify_chain`/`verify_operation_facts`
   au `#9` de `envelope.rs` (résolution feuille #7, signature #8, chaîne
   #9, `covers()` anti-abus #10). Rejeu **byte-exact** contre `p1` — les 5
   cas P1-deferred passent verts.
4. **A.4/A.5** : PUT `manifest`/`gamma`/`certs` + **CAS des deux têtes** en
   transaction atomique opaque ; publish/édition → `verify_for_cas()` ;
   `/gamma` → vérif d'entrée core. Le store n'arbitre jamais un fork.
5. **Heads / batch / sync** (A.3).
6. **Backend durable** : S3 (objets opaques) + DynamoDB (têtes CAS) derrière
   les seams (`objects.rs` + nouveau seam CAS). **Trancher ici
   Lambda-vs-Fargate pour le store** (gate P2, cf. INFRA-PROVIDER §7 note
   gravée). Le relais reste Fargate.
7. **Témoin** sur le head canonique (annexe C ; `witness.rs` écrit, non
   composé ; clé KMS Ed25519 sign-only).
8. **Vrai E2E** : bundle grantee → HTTP provider → arrêt/restart →
   téléchargement dans un store vierge → **cold verify** → lectures
   owner/grantee. Aucun mock du protocole.

## OÙ

- Code : `code/aithos-core` branche `feat/obligations`, crate
  `rust/crates/aithos-provider`. Bundle façade
  `rust/crates/aithos-bundle/src/publication.rs`. Vecteurs `vectors/`.
- Provider infra : `provider` branche `feat/p6-p7-tunnel` —
  `infra/terraform`, `e2e` (behave), CI plan-only.
- ⚠️ `cargo` absent de la VM device (pas de réseau) : **stager le crate vers
  le sandbox cloud** pour compiler/tester, puis réécrire in situ par `cp`
  (le mount bloque `unlink` → `tar x` échoue sur l'existant ; extraire dans
  un temp puis `cp -R temp/. dest/`). Musl statique via `cargo-zigbuild` +
  `zig` (le CDN alpine n'est pas joignable). Images `FROM scratch`
  hand-assemblées via `crane` (append `--oci-empty-base` puis `mutate
  --entrypoint --set-platform linux/amd64`). L'egress du sandbox
  **intercepte le TLS brut** (proxy « Anthropic Egress Gateway ») : toute
  sonde TLS/ALPN réelle = **vraie machine**, pas le sandbox.

## TESTER

- `cd code/aithos-core/rust && cargo test -p aithos-provider --features pod-stub`
- `cd code/aithos-core/vectors && python3 gen-p.py && python3 verify-p.py`
  (p1..p6 doivent rester **byte-identiques** ; p7+ s'ajoutent à côté)
- `cargo test -p aithos-core -p aithos-bundle --locked` (la façade ne doit
  jamais régresser)
- wire e2e : `E2E_BASE_URL=https://store.aithos.fr behave provider/e2e/features`
- rejeu déployé : `AITHOS_REPLAY_URL=https://store.aithos.fr cargo test -p
  aithos-provider --test vectors_replay -- --nocapture`

## GOTCHAS

- Creds Mathieu dans `/Volumes/Math17/aithos/v2/.aws-env` (SSO, expire) —
  exporter pour AWS/terraform, **purger après**, jamais dans un log/dépôt.
  Vérifier `AWS_CREDENTIAL_EXPIRATION` **dans le futur** avant usage.
- Backend S3 tfstate `aithos-landings-tfstate-128066560720`, région
  `eu-west-3`. `terraform init -reconfigure -backend-config=…` (cf.
  `envs/prod/README.md`).
- task def Fargate DOIT porter `AWS_REGION` ; image `FROM scratch` DOIT
  embarquer le bundle CA. `desired_count` relais **intouché**.
- Le CAS **provider** (têtes A.5, DynamoDB, transaction serveur) ≠ la
  transaction **bundle** locale (G-B). Ne pas les confondre.
- Vecteurs gelés (`p1..p6`, `cb2-*`) : un changement = **nouveau id +
  redline d'annexe par gate**, jamais une modif en place.

## NORMATIF

INFRA-PROVIDER **annexe A** : A.2 (enveloppe `X-Aithos-Auth`, ordre 0–10,
fail-closed), A.3 (routes + path-map `covers()` anti-abus), A.4
(vérification d'artefacts au dépôt — déléguée à core/bundle), A.5 (CAS des
deux têtes chaudes : manifest `chain_hash`, gamma head), A.7 (registre
d'erreurs fermé), A.8 (limites + discipline de logs). Annexe C (témoin).
JCS RFC 8785, Ed25519 sur JCS-avec-`signature.value=""`, clés multibase
`z6Mk…`, BLAKE3, RFC 3339 Zulu.

## PREMIÈRE ACTION

Lire l'état (ce prompt + HANDOFF-PROVIDER-P2-RESUME + INFRA-PROVIDER annexe
A + la façade `publication.rs` in situ + les points de défer `envelope.rs`
#9 / `service.rs`), **confirmer le cadrage à Mathieu**, puis écrire les 2
features + leurs vecteurs rouges AVANT toute ligne de code. STOP au gate
contrat. Ne réimplémente aucune règle core/bundle : appelle la façade.
