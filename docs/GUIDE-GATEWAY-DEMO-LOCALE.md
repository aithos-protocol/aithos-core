# Guide — gateway locale sur le provider réel : démo de bout en bout et gestion

> **Statut : référence historique DEV.** Ce guide décrit DEMO-LEA en loopback
> avant la qualification G4. Ses preuves provider restent utiles, mais ses
> limites G1/OAuth ne décrivent plus l'état courant. Le runbook G4 et
> `CLI-DELEGATED-OAUTH.md` priment pour toute session déléguée.

Date : 2026-07-21. Public : l'opérateur de la démo (Mathieu).
Ce guide consolide et complète `DEMO-LEA-PROVIDER-CLI.md` (le runbook de
référence, vérifié conforme au code ce jour) : il ajoute les
prérequis contrôlés, les preuves côté provider à montrer pendant la
démo, la gestion de la gateway au quotidien et le dépannage. En cas
d'écart entre les deux documents sur une commande, le runbook prime —
signaler l'écart.

Le cas d'usage joué est DEMO-LEA (`DEMO-LEA-SCENARIO.md`) : l'agente
immobilière Léa, un contexte `ventes` gouverné (3 connecteurs, bornes,
directive), un journal d'agent — le tout avec le **provider réel**
(`store.aithos.fr`) : journal en mode B (le provider est le primaire),
ventes en mode A (fs primaire + réplication). Données synthétiques,
tenant jetable, purge en fin de séance.

---

## 1. Prérequis (5 min, une fois)

Sur la machine de démo :

- Rust stable (`cargo --version` ≥ 1.95), Docker (pour le Vault dev),
  `openssl`, `awk`, `curl`, `jq` (confort).
- Le dépôt `code/aithos-core` à jour, batterie verte (au moindre doute :
  `cargo check --workspace --locked` depuis `rust/`).
- **Creds AWS fraîches** pour les gestes admin (création/purge du
  tenant) : `aws sso login --profile aithos-prod` juste avant la séance.
  ⚠ Le fichier `.aws-env` expire (~1 h) — s'il a plus d'une heure,
  le régénérer plutôt que de « réessayer ». Les creds ne servent QUE
  dans le terminal admin (jamais dans la gateway, jamais dans le yaml).
- Réseau sortant HTTPS vers `store.aithos.fr`, `witness.aithos.fr`,
  `public.aithos.fr`.

Sondes d'entrée (tout doit répondre 200) :

```bash
curl -s -o /dev/null -w '%{http_code}\n' https://store.aithos.fr/healthz
curl -s -o /dev/null -w '%{http_code}\n' https://witness.aithos.fr/keys.json
curl -s -o /dev/null -w '%{http_code}\n' https://public.aithos.fr/healthz
```

## 2. Le déroulé — suivre le runbook, section par section

Suivre `DEMO-LEA-PROVIDER-CLI.md` §0 → §10 dans l'ordre. Rappel de la
topologie des terminaux : ① Vault, ② ×3 MCP synthétiques, ③ gateway,
④ owner/démo (+ ⑤ « preuves provider », voir §3 ci-dessous).

Résumé des étapes et de ce qu'elles PROUVENT (les commandes exactes sont
dans le runbook) :

| § runbook | Geste | Ce que ça démontre |
|---|---|---|
| 0–1 | build + session jetable | rien n'est installé côté Aithos |
| 2 | Vault dev + 3 bearers | les pleins pouvoirs ne vivent QUE dans le coffre de l'entreprise |
| 3 | 3 MCP permissifs | les amonts n'imposent AUCUNE borne — tout ce qui sera refusé le sera par la gateway |
| 4 | keygen, init journal + ventes, discovery, enrollment, briefing | tout le provisioning est un geste OWNER local ; le master seed ne quitte jamais ce terminal |
| 5 | tenant + `bind-did` + `owner-replicate-history` ×2 | l'état signé part chez le provider PAR LE WIRE ; reprenable, fail-closed |
| 6 | `demo-lea-render-config` | le yaml ne contient AUCUN secret (le grep doit rendre 0) |
| 7 | `run` + beats 1–6 | surface exacte du mandat, refus pédagogiques, bearers injectés par la gateway seule |
| 8 | beat 7 : hot edit + re-réplication | le caractère est gouverné, pas compilé — et la nouvelle édition part au provider |
| 9 | beat 8 : audit-export | l'auditeur rejoue tout depuis le gamma, dans son périmètre seulement |
| 10 | purge | il ne reste RIEN : tables 0, S3 vide |

Points d'attention vérifiés dans le code :

- `owner-replicate-history` est **reprenable** : le relancer ne renvoie
  que le delta, et il REFUSE si le remote est en avance ou si le DID ne
  correspond pas au couple kind/label. C'est le comportement attendu,
  pas une erreur.
- Le beat 7 se termine par une **re-réplication** (runbook §8) — sans
  elle, le provider sert encore l'édition précédente de ventes (mode A :
  l'édition owner est un geste externe à la gateway).
- `audit-export` fonctionne sur les stores remote/replicated depuis le
  gate P3 (l'identité du pod est requise — `--identity` obligatoire).

## 3. Le 5e terminal — les preuves côté provider (le « deuxième écran »)

C'est ce qui transforme la démo technique en argument produit : pendant
que l'agent travaille en local, TOUT se prouve depuis l'extérieur, sans
la gateway, sans clé.

Après le seed (§5 du runbook), puis à volonté pendant les beats :

```bash
# Le témoin a observé les publications (checkpoints signés, publics) :
curl -s "https://witness.aithos.fr/$JOURNAL_DID.jsonl" | tail -2 | jq .
curl -s "https://witness.aithos.fr/$CONTEXT_DID.jsonl" | tail -2 | jq .

# Les têtes chaudes du journal, servies par le store (enveloppe requise
# pour /heads — utiliser le lecteur owner si besoin) ; la surface
# ANONYME, elle, se montre sans rien :
curl -s "https://store.aithos.fr/t/$TENANT/$CONTEXT_DID/did.json" | jq .
curl -s -D- -o /dev/null "https://public.aithos.fr/t/$TENANT/$CONTEXT_DID/did.json" | grep -i 'x-cache\|cache-control'
```

À montrer au fil des beats :

- après le **beat 4** (mail envoyé) : le beat suivant du témoin sur le
  journal (chaque publish → checkpoint public en ~6 s) ;
- après le **beat 7** (hot edit + re-réplication) : la nouvelle édition
  observée par le témoin sur `$CONTEXT_DID` — la gouvernance est
  PUBLIQUEMENT horodatée ;
- à tout moment : deux checkpoints incompatibles n'existent nulle part —
  c'est l'anti-équivocation, l'argument « vous n'avez pas à nous
  croire ».

Vocabulaire pour l'audience : « le contenu et la preuve viennent
d'aithos.fr ; la décision et l'action restent chez le client. »

## 4. Gérer la gateway (au-delà de la démo)

**Cycle de vie.** La gateway est un processus sans état précieux : tout
ce qui compte vit dans les Ethos (et chez le provider en mode B). Le pod
ne détient que `agent.id` (seeds agent+gateway) et le yaml. Arrêt :
Ctrl-C / SIGTERM. Relance : la même commande `run`. En mode B, une
gateway arrêtée = actes refusés (fail-closed), rien n'est perdu.

**Chaud vs redémarrage.** Règle : *si le geste change qui peut faire
quoi, c'est chaud ; s'il change ce que le pod expose, c'est un restart.*

- À CHAUD (aucune interruption) : éditer une directive/briefing
  (`owner-set-briefing`), frapper un mandat, révoquer, atténuer, les
  sessions OAuth. En mode A, penser à `owner-replicate-history` après un
  geste owner pour que le provider converge.
- RESTART (quelques secondes) : ajouter un contexte au yaml, exposer de
  nouveaux outils dans un contexte (après `owner-enroll-server`),
  changer un endpoint de serveur MCP.

**Surveillance minimale.** La gateway logge sur stderr (`gateway
listening on …` au boot). Santé du provider : les trois sondes du §1.
État du tenant : `aithos-store-admin` (suspend/reactivate agissent sur
le wire en < 60 s). Le journal de vérité reste le gamma — consultable à
tout moment par `audit-export` (auditeur) ou par un lecteur owner.

**Secrets.** Le yaml ne porte que des références (broker Vault, path,
field). Le token Vault arrive par variable d'environnement au `run`.
Aucun bearer amont ne doit jamais apparaître dans un fichier, un log ou
une sortie de terminal — le grep du §6 du runbook est le contrôle.

## 5. Dépannage (les refus sont des diagnostics, pas des pannes)

| Symptôme | Cause probable | Geste |
|---|---|---|
| `503 unavailable` au seed ou au run | backend provider injoignable côté service (fail-closed) — ou tenant pas encore créé | vérifier §1 sondes ; `$ADMIN create`/`bind-did` faits ? |
| `404 unknown_tenant` | tenant absent de la table control | `$ADMIN create "$TENANT"` |
| `403 did_not_bound` | DID non lié au tenant | `$ADMIN bind-did "$TENANT" <did>` (journal ET contexte) |
| `403 suspended` | tenant suspendu | `$ADMIN reactivate "$TENANT"` (servi < 60 s) |
| `401 clock_skew` | horloge locale décalée > 300 s | resynchroniser l'horloge machine |
| `403 chain_invalid` / `not_covered` | mauvais mandat dans le yaml, ou fenêtre expirée | vérifier `CONTEXT_MANDATE`/`MEMORY_MANDATE` (les valeurs de `enroll.out`/`journal.out`) |
| `409 cas_mismatch` au replicate | le remote a avancé (ou re-run croisé) | relire le message : `owner-replicate-history` refuse si le remote est EN AVANCE — ne jamais forcer, comprendre qui a écrit |
| replicate refuse « did mismatch » | mauvais couple `--kind/--label` vs store-root | reprendre les exports du §4 du runbook |
| la gateway boot mais refuse un outil du yaml | yaml ≠ manifest scellé (garde fail-closed) | ré-enrôler ou corriger le yaml — la gateway ne « répare » jamais |
| beat 6 ne montre pas le nouveau texte (beat 7) | édition faite, gateway OK, mais… mauvaise zone/label | l'édition circle est servie à la lecture SUIVANTE, sans restart — vérifier la commande §8 |
| creds AWS « expired » dans le terminal admin | `.aws-env` périmé | `aws sso login --profile aithos-prod` et re-sourcer |

Règle générale : chaque refus porte un code du registre A.7 — le code
DIT la cause. Rien ne s'écrase, rien ne se répare en silence ; c'est le
comportement contractuel.

## 6. Fin de séance — l'état de repos

```bash
$ADMIN purge "$TENANT" --yes     # versions S3 → heads → control, en ordre
docker stop aithos-lea-vault
test "$DEMO" = /tmp/aithos-lea-demo && rm -rf -- "$DEMO"
```

Contrôle : tables control/heads à 0, préfixe `t/$TENANT/` vide. Résidu
assumé et inoffensif : les lignes de checkpoint du témoin sur les DIDs
jetables (feed append-only par design — les renier casserait la racine
quotidienne) et les nonces TTL (~15 min).

## 7. Limites connues de cette démo (à dire si on te pose la question)

- L'agent parle à la gateway en **loopback** : l'entrée par
  `<entreprise>.mcp.aithos.fr` (relay) attend le chantier G1 côté
  binaire gateway — le relay lui-même est déployé et prouvé.
- Les 3 connecteurs sont synthétiques ; les vrais (TLS/OAuth) sont le
  lot suivant de la verticale SDK.
- Vault est en mode dev ; un Vault scellé + AppRole est reporté.
- Quotas, GC/rétention, DR : lot C ops.
