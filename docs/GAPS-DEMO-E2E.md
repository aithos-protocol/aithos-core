# GAPS — Analyse bout-en-bout : ce qui manque pour la démo et le produit

> **ARCHIVE D'ANALYSE — 16 juillet 2026.** Plusieurs écarts ont depuis été
> fermés par G4, le SDK v2, OAuth SaaS et le Provider. Pour la démo courante,
> utiliser `HANDOFF-GATEWAY-COMPAGNON-DEMO-INTEGREE-2026-07-22.md`.

> **Statut : ANALYSE VALIDABLE — 2026-07-16.** Passe en revue `aithos-core`, la
> gateway et les deux plans d'action ([P](HANDOFF-PROVIDER-AWS.md),
> [G](HANDOFF-GATEWAY-HUB.md)) contre le scénario de démo cible, puis contre le
> produit final. Chaque trou est rattaché à un lot existant ou marqué **décision**.
> Légende : ✅ existe et prouvé · 🔶 planifié (lot) · 🔴 trou découvert par cette
> analyse.

## 1. Le scénario de référence (démo BYO)

Tel que formulé par Mathieu (2026-07-16), annoté des recommandations §4 :

1. L'entreprise configure sa gateway avec ses connecteurs (Notion, Gmail),
   tokens dans son coffre.
2. Elle crée **un Ethos**, y branche les connecteurs, remplit les trois zones
   (`public` : présentation ; `circle` : mémoire commerciale, consignes ;
   `self` : notes owner — **reco : `self` reste owner-only**, voir §4.2).
3. Elle frappe **un mandat** pour une assistante commerciale : lecture de
   l'Ethos (`public` + `circle`), outils Notion (lecture) et Gmail
   (`send_email` **borné** : `to ∈ liste de prospects approuvée`).
4. Elle envoie le mandat par mail (pack d'invitation — §4.1).
5. L'assistante ouvre Claude Cowork, ajoute le connecteur
   `<entreprise>.mcp.aithos.fr`, se connecte **avec son mandat** (cérémonie
   OAuth → sous-mandat de session).
6. Elle demande : « prends la liste des prospects et envoie-leur un mail pour un
   RDV ». Claude **lit l'Ethos** (contexte), appelle Notion, rédige, envoie.
7. **Refus pédagogique** : des destinataires hors liste → erreur nommant le champ,
   les intrus et la liste approuvée ; zéro hit coffre, zéro hit Gmail. Claude
   corrige, ré-envoie aux seuls autorisés → succès.
8. Côté entreprise : on ouvre la **page de preuve**, on lit le gamma — actes,
   refus, chaînes, comptages. (**Reco bonus §4.3 : beat de révocation live.**)

Verdict global : **le scénario est bon, clair, et structurellement identique à
DEMO-LEA transposée en BYO** — la mécanique la plus risquée (bornes, refus
pédagogique, log-before-relay, coffre) est déjà prouvée par les beats 2–4 de Léa.
Trois vrais trous et deux décisions, ci-dessous.

## 2. Couverture beat par beat

| Beat | Composants | État |
|---|---|---|
| 1. Gateway + connecteurs + coffre | config v3, `owner-discover/enroll-server`, broker Vault KV v2 | ✅ (DEMO-LEA) |
| 2. Créer l'Ethos, remplir les zones | CLI core (zones, sections, tags) ; UX = CLI assumée v1 | ✅ (friction, pas trou) |
| 3a. Mandat lecture Ethos + outils | grants zones (pass L) + `act.*` + un seul mandat multi-périmètres | ✅ core / 🔶 **G8.c** (surface d'émission multi-mandats propre) |
| 3b. Borne Gmail `to one_of` | manifest scellé, lot P | ✅ |
| 4. Envoi du mandat par mail | pack d'invitation | 🔴 → **G4** (+ décision §4.1) |
| 5a. Connecteur Claude → hub public | tunnel + TLS + SNI | 🔶 **G1/G2 + P6/P7** |
| 5b. OAuth (DCR/CIMD, PKCE, consentement) | `gateway_as` | 🔶 **G3** |
| 5c. Session = sous-mandat | cérémonie wasm + multi-principal | 🔶 **G4/G5** |
| 6a. **Claude lit l'Ethos** | outils natifs de lecture de sections | 🔴 → **G6** (trou : seuls `journal.*` et `briefing.read` existent) |
| 6b. Notion lecture | hub relay + grants | ✅ |
| 7. Refus pédagogique + retry | bornes lot P, `bound_violated` détaillé | ✅ |
| 8. Page de preuve entreprise | surface owner/auditeur HTTP + vérif wasm navigateur | 🔴 → **G7** (CLI `audit-export` existe mais peu démonstrative) |

## 3. Trous transverses (produit final, au-delà de la démo)

### 3.1 Parité owner/mandat — le point historique
Le core a déjà : écritures déléguées toute-lattice (pass L), sous-mandats
offline, révocation par ancêtres/watchdog, N mandats vers une même clé. Restent,
tracés dans `MANDATES-PRODUCT-GAPS.md` et repris en **G8** :
- **`id=` de section absent** — bloque les écritures propres sur `self`
  (`dir=`/`tag=` y sont read-only par design, §10.7.6) et les mandats
  ultra-fins ;
- **atténuation incomplète** : `verify_chain` ne vérifie que fenêtres +
  obligations — un sous-mandat pourrait élargir `max_actions`/`action_params` ;
  à fermer avant tout usage sérieux des sous-mandats de session (G3–G5 en
  dépendent : **G8.b passe avant le gate de G5**) ;
- **émission multi-mandats** depuis un Ethos : mécanique OK, surface produit
  manquante ;
- **composition** borne-manifeste ∧ restriction-mandat : l'intersection doit
  s'appliquer et être testée.
P1 (plus tard, à garder tracé) : wildcard `act.x.<c>.*` vs classe `binding`,
read-model actif/expiré/révoqué.

### 3.2 Zones, clés, sections — anticipations demandées
- `public` : plaintext, sans clé ni header — un mandat « full Ethos » la couvre
  trivialement ; rien à faire.
- `circle` : lignes de header vers la clé du grantee ; via le hub, la **physique**
  est la ligne de la gateway, l'**autorité** la chaîne de session, la **trace**
  une entrée `ethos.read` sous cette chaîne (G6). Lecture e2e sans gateway
  (dashboard/humain) : via RemoteStore + déchiffrement local (P2–P3).
- `self` : structure scellée, sids opaques ; lecture délégable, écriture par
  `id=` (dépend de G8.a) ou grant de zone. **Reco produit : ne pas inclure `self`
  dans les mandats externes par défaut** — c'est la démonstration vivante de la
  différence entre zones.
- Rotation/révocation opérables par CLI ✅ ; dettes différées connues (re-seal
  descendant post-move, tag-views déplacées) : inchangées, hors démo.

### 3.3 Multi-mandats simultanés
N sessions sur **un** runner = G5 (sérialisation in-process du gamma ✅). N
runners actifs sur **un même contexte** = attend le RemoteStore CAS (P3) — limite
v1 documentée (`HANDOFF-MANDATES-SURFACE` §1.d), levée par la piste P.

### 3.4 Divers produit
- **Fraîcheur/anti-équivocation** : témoin P5 ; TTL courts par défaut sur les
  sous-mandats de session (G4).
- **Perf `gamma_full_verify` 10k = 538 ms** (cible 200 ms) : sans impact démo ;
  décision batch-verify différée, inchangée.
- **LLM** : dans le flux BYO, le LLM est celui de l'utilisatrice (Claude) —
  `proxy_llm` et V4 (credential LLM au vault) sont **hors de ce flux** ; les
  budgets qui gouvernent = actions/bornes, pas tokens. À dire honnêtement dans le
  pitch.
- **Packaging** : image gateway signée + doc d'install client + runbook démo
  (G9) ; hashes notarisés par le témoin (P5).
- **Logistique Claude** (vérifiée 2026-07-16) : custom connectors = OAuth
  DCR/CIMD, bearer statique refusé, callback `claude.ai/api/mcp/auth_callback`,
  consentement obligatoire ; prévoir un compte Pro/Max (ou Team avec droits
  admin) pour la démo ; option : allowlister l'IP egress Anthropic
  (`160.79.104.0/21`) au relay.
- **Streamable HTTP réel** : notifications, sessions, GET SSE — G2 (petits
  correctifs probables, à tester contre Inspector avant Claude).

## 4. Décisions

### 4.1 Livraison du mandat (chicken-and-egg des clés) — **décision prise, à assumer**
Un mandat se frappe **vers une pubkey** ; or l'assistante n'a pas encore de clé
quand l'owner frappe. Deux flux gravés en G4 :
- **Pack d'invitation (DÉMO / DEV)** : l'owner génère keypair + mandat, envoie le
  pack par mail. Simple, une seule étape — mais la custody a voyagé par mail et
  l'entreprise a « tenu » la clé de l'assistante (non-répudiation affaiblie).
  Acceptable en démo, **marqué DEV, jamais un défaut de prod**.
- **Pubkey-first (PROD)** : l'invitation ouvre la cérémonie, la clé naît dans son
  navigateur, la pubkey remonte, l'owner frappe (une validation asynchrone).
  Non-répudiation pleine, custody jamais partagée — la philosophie §3bis.2
  (« la clé naît là où elle vit ») étendue aux humains.

### 4.2 `self` dans le mandat « full accès » — **reco : non**
Full accès = `public` + `circle` + outils. `self` reste owner-only dans la démo :
c'est le moment le plus parlant (« la note de marge ne sortira jamais, même avec
le mandat le plus large »). L'inclure est protocolairement possible si un client
le veut — choix, pas limite.

### 4.3 Beat de révocation — **reco : oui, en clôture**
Après la beat 8 : l'owner révoque le mandat en live ; l'appel suivant de
l'assistante est refusé, le refus est journalisé. Dix secondes, et c'est la
moitié de la promesse (« lui retirer l'accès sans fermer l'infrastructure »).

### 4.4 Nommage — **gravé**
`<org>.mcp.aithos.fr` (A6) : requis techniquement par le passthrough SNI, et
c'est la forme la plus lisible commercialement.

## 5. Fausses alertes (vérifiées, pas des trous)

- Le token Gmail « large » : coffre client, résolution par appel, jamais exposé —
  la restriction se joue aux bornes, c'est le design (et l'argument de vente).
- « Aithos voit passer les données » : non — passthrough SNI (A3), le relay est
  prouvé aveugle au gate P6.
- Écritures déléguées / verbes : pass L complet côté core.
- Comptage multi-mandats/sous-mandats : règle de sous-arbre, déjà implémentée et
  testée (`count_actions`).
- Concurrence d'éditions : merge disjoint + fork rule spec §02.6, étape I close.

## 6. Ordre de bataille résumé

**Chemin critique démo** : G8.b (atténuation) → G3→G5 (OAuth + sessions) + G6
(lecture Ethos) + G7 (preuve) + G4 (invitation), avec P6/P7 (relay + tenants) en
face, P0/P1 en socle. **Parallèle sans risque** : P2–P5 (store + témoin), G8.a/c/d.
Tout le reste du produit (console, RemoteVault, fédération, hardening pack) reste
hors périmètre et tracé dans INFRA-PROVIDER §9.
