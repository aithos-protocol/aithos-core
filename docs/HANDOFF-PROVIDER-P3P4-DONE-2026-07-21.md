# HANDOFF — Lot B (P3/P4), partie 2 : GATES P3 + P4 DÉPLOYÉS — FAIT (2026-07-21)

Date : 2026-07-21. Dépôts : code/aithos-core + provider/. État DISQUE = vérité.
Statut : **le lot B est CLOS — les deux gates sont déployés et passés au
témoin.** Gate P3 : DEMO-LEA rejouée À L'IDENTIQUE avec journal remote
(mode B, binaire gateway, pen mémoire) et ventes replicated (mode A, relu
du store) ; extension micro-redline A.1 (porteurs connecteurs) exigée et
servie ; gate déployé 35/35 sur DID frais, behave 25/25, plan 0. Gate P4 :
client `/sync` + `/batch` (BDD 21/21), module `cdn-public` APPLIQUÉ
(public.aithos.fr, 6 add/0/0, plan final 0), gates perf §3.6 mesurés
(pré-mesure sandbox ; script officiel FOURNI pour ta machine). Témoins
adversariaux en agent ×2 : « le vert n'est pas réfuté », **0 bloquant**.
Les commits restent TON geste (blocs §5). Session conduite en DÉLÉGUÉ
TOTAL (ton choix AskUserQuestion du matin), tout consigné.

Se lit après HANDOFF-PROVIDER-P3-JALON-CLIENT-2026-07-21.md ; gravures
dans INFRA-PROVIDER.md (§3.5 note « modes réalisés », §3.6 chiffres, §7
note cdn-public, annexes A.1/A.3/A.6).

## 0. Séquence (délégué total, motif P7b — décisions consignées)

1. Rituel d'entrée VERT (git bb31d71/46399b0, wire 200×3, AWS 0/0 + 2/2+1/1+1/1).
2. **Décisions du matin (AskUserQuestion)** : C6 = `replicate_history`
   reste TEST-ONLY (promotion consignée lot C) ; mode = délégué total.
3. Sandbox reconstruit (~300 fichiers, .feature par git-objects). **Deltas
   disque irrécupérables par le pont** (2 fichiers .feature refusés au
   staging, deltas non committés) : reconstruits ÉQUIVALENTS — comptes du
   handoff retrouvés à l'exact (remote 16/16 à 142 steps) ; consigné,
   validé par le témoin (« la reconstruction n'affaiblit aucun contrat »).
4. Batterie d'entrée complète verte, puis gate P3 : RED e2e (porteurs
   connecteurs hors grammaire + audit-export sans identité) → code
   minimal → VERT → image → rejeu wire → behave → purge → tables 0.
5. Témoin P3 (agent) : « non réfuté », 0 bloquant, C1 amendé.
   Gravures INFRA-PROVIDER (5).
6. Gate P4 : BDD RED 5 scénarios → client sync/batch VERT (21/21) →
   cdn-public plan lu INTÉGRALEMENT (6 add/0/0, rien hors lot) → apply →
   bench §3.6 : **leçon serveur** (pack /sync servi en 1 000 GETs S3
   séquentiels ≈ 26 s) → correctif concurrent borné 64 ordre préservé →
   2,36 s → image P4 redéployée → pré-mesures gravées.
7. Témoin P4 (agent) : « non réfuté », 0 bloquant, 3 consignés.
8. Write-back disque de TOUT, repos (tables 0, S3 t/ vide, rôle de
   session supprimé).

## 1. Preuves (2026-07-21, sandbox ; wire public pour la prod)

| Preuve | Résultat |
|---|---|
| cargo check --locked workspace | EXIT=0 |
| cucumber | store **151/151** (992) = 148 + 2 redline-A.1 + 1 porteurs-connecteurs ; tunnel 12/12 ; relay 27/27 ; witness 12/12 |
| **remote (cucumber_remote)** | **21/21 (195)** = 16 P3 + **5 P4 (batch ×2, sync ×3 — RED constaté avant code)** |
| replays byte-exact | vectors 5/5, p3 2/2, p5 1, p6 2/2, handshake 4/4, witness p4 3/3 — vecteurs INTOUCHÉS |
| gateway | lib 85/85, cucumber 152/152, **e2e_demo_lea 2/2 (fs + REMOTE mode B)**, e2e_journal_remote 2/2 ; bundle 815/815 ; clippy = les 2 warnings préexistants C5 seuls ; rustfmt clean |
| **remote_cache_nav (gate §3.6 cache local)** | p50 **0 µs** / 1000 hits, **1 seul fetch wire** (immutable jamais re-demandé) |
| **gate déployé P3** (deployed-replay-p3.py, tenant jetable, **DID FRAIS** D8) | **35/35 GREEN** : base étape 6 + treillis appendeur + redline A.1 (header/root, classes+ETag+304) + porteurs connecteurs + exclusions runner (gateway/**, manifests/tree-*, e/x/root.enc → path_invalid) |
| behave (store-p1, control-p7, witness-p5, store-acme-p6) | **25/25** (suspension 403 < 60 s et réactivation < 60 s MESURÉES sur le tenant de gate) |
| terraform | P3 : plan final **0** (delta image seule) ; P4 : plan lu **6 add/0 change/0 destroy** (tout module.cdn_public), apply OK, plan final **0** |
| **bench §3.6 (pré-mesure sandbox, RTT 111,5 ms)** | cache local ~0 µs GREEN ; CDN immuable **20,7 ms** GREEN (section publique 21,6 ms) ; append 229 ms (part serveur ≈ 115 ms — cible UE à la marge, consigné) ; sync froid **2,36 s** (26,3 s avant correctif ; cible 2 s à confirmer à l'officiel) |
| état AWS à la clôture | control **0**, heads **0**, S3 `t/` **0 version** ; store 2/2, relay 1/1, witness 1/1 ACTIVE ; store/public/witness wire 200 ; AUCUN checkpoint sur le DID frais du rejeu P3 (aucun publish) |
| témoins adversariaux (agents, re-comptes indépendants) | P3 : « le vert n'est pas réfuté », D8 réglé par construction ; P4 : « le vert n'est pas réfuté » — **0 bloquant** aux deux |

Images ECR store-api : P3 `sha256:224be505…` (`:prod` puis remplacée),
**P4 `sha256:cec2c667…`** (`:prod` + `:bb31d71-p4`, EN SERVICE — rollout
COMPLETED 2/2 ×3 ce jour ; repli P3 : tag `:bb31d71-p3`).

## 2. Ce qui a été construit (tout sur le disque)

- **e2e_demo_lea.rs paramétré** : un seul corps `dress_rehearsal(mode)`,
  beats identiques (AUCUNE branche dans les beats — vérifié témoin) ;
  variante remote = service provider réel in-process, seed owner PAR LE
  WIRE (motif du spike, test-only C6), gateway BINAIRE en yaml
  `journal: {kind: remote, …, mandate: [<memory_mandate>], local: <dir>}`,
  ventes `{kind: replicated}` ; assertions finales par lecteur REMOTE
  indépendant + preuve « 0 beat sur le disque du pod » + convergence du
  sweep ventes re-lue du store. Fakes anti-flaky (bind :0 direct).
- **Service (redline A.1 étendue au gate)** : `ConnectorHeader`/
  `ConnectorConfig` (`e/x/<id>/header.json|manifest.enc`) — grammaire,
  couverture `act.x.<id>.*`/owner, classe private-revalidate + ETag,
  A.4 léger. BDD : +1 scénario porteurs (et les 2 redline reconstruits).
- **Binaire gateway** : `audit-export` construit son store via
  `from_config_with_identity` (replicated → primaire fs, remote → wire ;
  aucune capacité nouvelle — vérifié témoin).
- **Client** : `RemoteStore::get_many` (POST /batch) et `::sync`
  (POST /sync) → `Vec<PackPart>` {path relatif, status par part, octets
  sur 200} ; parseur multipart (boundary du Content-Type, fail-closed
  typé) ; 410 → `RemoteError::Wire{edition_gone}`.
- **Service (P4 perf)** : `fetch_pack_bodies` — corps des packs
  batch/sync fetchés CONCURREMMENT (buffered(64), ordre préservé,
  couverture décidée AVANT, erreur backend = refus du pack entier).
- **Infra** : module `cdn-public` (CloudFront public.aithos.fr, origine
  = LE SERVICE — surface structurellement anonyme, CachingOptimized sur
  les classes A.6, cert us-east-1, alias A+AAAA) + câblage envs/prod.
- **Outils de gate** : `vectors/deployed-replay-p3.py` (35 checks, DID
  frais par graine env), `vectors/bench-p4.py` (bench officiel — voir
  §4), `tests/remote_cache_nav.rs` (gate cache local).

## 3. Verdicts témoins et consignés (AUCUN bloquant)

Gravés ce jour (INFRA-PROVIDER) : micro-redline A.1 + extension
porteurs ; convention JCS client ; A.3 treillis (C2) + gamma-appendeur
(C1 AVEC AMENDEMENT : la largeur « tout l'historique pour tout
appendeur » excède le besoin CAS — resserrer un jour au(x) segment(s)
que la chaîne étend) ; A.6 porteurs + 304 ; §3.5 modes A/B réalisés ;
§3.6 chiffres ; §7 cdn-public.

Consignés SANS graver (repris au lot C sauf mention) :
- **E1 (P4)** : parseur multipart client sans Content-Length — sûr pour
  les classes JCS servies, à durcir par cadrage par longueur.
- **E2 (P4)** : MAX_PACK_BYTES borne la réponse, pas la RAM de
  l'assemblage (borné en pratique par batch ≤ 256) .
- **E3 (P3)** : perf append serveur ≈ 115 ms (nonce → tête → segment →
  transact séquentiels) — pipeliner au lot ops si l'officiel est RED.
- **E4 (P4)** : sync froid 2,36 s vs cible 2 s — pistes : streaming de
  la réponse multipart, pool backend chaud.
- **E5** : `witness.aithos.fr/healthz` répond 403 (surface statique par
  design — pas de sonde santé témoin) ; healthz du store caché à l'edge
  via public.aithos.fr (sans Cache-Control — inoffensif).
- **E6** : les objets `immutable` d'un tenant purgé peuvent survivre en
  cache edge — « résidu zéro » à nuancer (contenu de test inoffensif).
- **E7** : `e/x/header.json` (ZoneHeader("x")) n'est couvrable que par
  l'owner en pratique (aucune ligne de mandat ne porte la zone x).
- C4 (sidecar/state.json non re-dérivé), D3/D5/D6 (P5), quotas/GC/DR
  (§8) : INCHANGÉS, au lot C.
- Reconstructions de session (validées témoin) : store-reads.feature
  (2 scénarios redline réécrits équivalents), store-remote-client.feature
  (not_covered re-ciblé e/self — la substance de C2), store_adapter.rs
  (2 `local: None` de tests — copie du pont périmée).

## 4. Le bench OFFICIEL (ton geste, machine Mathieu — arbitrage ③)

```
cd code/aithos-core
# 1) le gate cache local (indépendant du réseau) :
cargo test -p aithos-provider --test remote_cache_nav -- --nocapture
# 2) le bench wire (tenant jetable, DID frais imprimé par le script) :
export AITHOS_ADMIN_CONTROL_TABLE=aithos-provider-prod-control
export AITHOS_ADMIN_OBJECTS_BUCKET=aithos-provider-prod-store-data
export AITHOS_ADMIN_HEADS_TABLE=aithos-provider-prod-heads
export AITHOS_BENCH_SEED=$(python3 -c "import os;print(os.urandom(32).hex())")
cd vectors
python3 bench-p4.py https://store.aithos.fr bench-$(date +%Y%m%d) --print-did-only
aithos-store-admin create bench-<date> && aithos-store-admin bind-did bench-<date> <did imprimé>
python3 bench-p4.py https://store.aithos.fr bench-<date>
aithos-store-admin purge bench-<date> --yes
```
Reporte les chiffres dans la note §3.6 (« chiffres OFFICIELS »). Si
append ou sync sont RED depuis chez toi : consignés E3/E4 = les pistes.

## 5. Commits (TON geste — blocs prêts)

```
cd code/aithos-core
git add rust/crates/aithos-bundle/src/remote.rs \
        rust/crates/aithos-provider/src/pathmap.rs \
        rust/crates/aithos-provider/src/service.rs \
        rust/crates/aithos-provider/tests/cucumber_remote.rs \
        rust/crates/aithos-provider/tests/remote_cache_nav.rs \
        rust/crates/aithos-provider/tests/features \
        rust/crates/aithos-gateway/src/main.rs \
        rust/crates/aithos-gateway/src/store_adapter.rs \
        rust/crates/aithos-gateway/tests/e2e_demo_lea.rs \
        vectors/deployed-replay-p3.py vectors/bench-p4.py docs
git commit -m "P3+P4: lot B clos — DEMO-LEA rejouée à l'identique en remote (mode B binaire gateway sous pen mémoire, seed owner par le wire ; ventes mode A relu du store ; lecteur final REMOTE, 0 beat sur le pod), micro-redline A.1 étendue aux porteurs vault des connecteurs (e/x/<id>/header.json|manifest.enc — exigée par la démo, BDD + prouvée wire), audit-export sous identité (replicated→fs, remote→wire), client /sync+/batch (PackPart, multipart fail-closed, 410 typé — BDD 21/21), packs servis en fetch concurrent borné 64 ordre préservé (sync froid 26,3 s → 2,36 s), gate cache local remote_cache_nav (p50 0 µs, 1 fetch wire) — gates déployés VERTS (rejeu 35/35 DID frais, behave 25/25, plans finaux 0), témoins adversariaux ×2 « non réfuté », gravures INFRA-PROVIDER (A.1/A.3 treillis+gamma-appendeur amendé/A.6+304/§3.5/§3.6/§7 cdn-public)"

cd ../../provider
git add infra/terraform/modules/cdn-public infra/terraform/envs/prod
git commit -m "P4: module cdn-public — CloudFront public.aithos.fr sur la zone anonyme du store (origine = le SERVICE, jamais le bucket : la décision A2 et les classes A.6 restent au store ; CachingOptimized, cert us-east-1, GET/HEAD seuls, aucun en-tête client transmis), câblage envs/prod ; plans du gate (6 add/0/0, final 0)"
```

(`envs/prod/plan-p4.txt` = le plan lu tel qu'appliqué ; `plan-p3-full.txt`
et `plan-p4-final.txt` = les plans finaux 0 des deux gates ;
`apply-p4.txt` = la trace d'apply.)

Push : la CI provider-image.yml reconstruira et re-poussera `:prod`+`:sha`
— sans conséquence (le contenu est celui déployé) ; le digest en service
est `sha256:cec2c667…`.

## 6. État AWS et nettoyage de session

- Rôle de session temporaire `aithos-ops-session-p3` (motif P5, consigné) :
  **DETACH + DELETE exécutés en clôture** — vérifie au besoin :
  `aws iam get-role --role-name aithos-ops-session-p3` → NoSuchEntity.
- Tenants de gate purgés : replay-p3-20260721, acme (behave), bench-p4-20260721
  (×6 itérations). Tables control/heads **0**, S3 `t/` 0 version.
- Résidu : nonces TTL auto-purgés (~15 min) ; lignes de feed témoin des
  DIDs jetables du bench (design C.3/D8, DIDs frais).

## 7. Prochain lot (la route v1 : P3/P4 ✓ → ops → dashboard)

**Lot C ops** (§8 + B.4) : bornage tuyau par tenant relay, quotas store,
rétention/GC 30 j configurable, DR testée, docs métadonnées/DPA, README
envs/prod ; consignés P5 : D3 signature-avant-dedup, D5
kms:SigningAlgorithm + key policy, D6 append-only IAM ; consignés de CE
lot : C4 (re-dérivation sidecar runner éphémère), C6 (promotion
`replicate_history`), amendement C1 (resserrer la lecture gamma
appendeur), E1–E4 (durcissements/perf ci-dessus). Puis **lot D**
dashboard (périmètre à proposer). Definition of done v1 :
PROMPT-REPRISE-PROVIDER-V1-FINALISATION-2026-07-20.md.

## 8. Environnement (delta session)

Sandbox reconstruit (~300 fichiers ; .feature par extraction git-objects
byte-exact SHA-1 vérifiés, 18 features racine comprises pour le bundle) ;
pont : 2 copies périmées détectées/corrigées (store_adapter tests,
scénario not_covered) — TOUJOURS vérifier le CONTENU ; disque du sandbox
saturé 2× (30 Go target/) — purge incremental + doublons de binaires ;
musl-tools installé, image par push-store-image.py (dérivé du modèle
witness, tolère LayerAlreadyExists au re-tag) ; terraform 1.13.5 ;
creds par rôle de session via MCP aws-api (2 assumes, 1 h) ; behave/
pynacl/base58/blake3/boto3 pip. `AITHOS_REMOTE_DEBUG=1` inchangé.
