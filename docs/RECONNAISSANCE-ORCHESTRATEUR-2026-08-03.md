# Reconnaissance avant premier run — 2026-08-03

Inventaire `Mac ↔ origin`, preuve de bootstrap dans le conteneur, rédaction des
amendements `AM-1` à `AM-5`, et décisions prises en séance.

> **Avertissement méthodologique.** Une première passe de cet inventaire a été
> écrite à partir d'un instantané servi par le pont device qui avait **environ
> trois heures de retard** : il montrait une copie de travail désynchronisée,
> une ref parasite et deux branches déjà supprimées. Tout cela était faux. Les
> faits ci-dessous proviennent d'une relecture fraîche à `09:59 UTC`, vérifiée
> par `git` en lecture seule (`GIT_OPTIONAL_LOCKS=0`, aucune commande écrivant
> dans `.git`). **Sur ce montage, un fait critique se revérifie avant d'être
> utilisé** — c'est une contrainte à porter dans l'outillage du fil, qui lira
> de toute façon depuis `origin` et non depuis le disque du Mac.

---

## 1. État réel du dépôt

Sain. Aucun verrou résiduel, copie de travail propre (hors les documents de
conception non suivis), merge du round 2 correct — deux parents, aucune perte
de contenu.

| Élément | État |
|---|---|
| `main` local | `72b96ec` — **13 commits en avance sur `origin/main`, 0 en retard** |
| `origin/main` | `db01690` (SPL-8, amputation) |
| Branches restantes | `main`, `codex/audit-c-headers`, `codex/bundle-publication-performance` |
| Copie de travail | propre, alignée sur `HEAD` (mtime `07:46:56`, celui du merge) |
| Verrous `.git/*.lock` | aucun |
| Worktrees | `/tmp/wt-b` et `/tmp/wt-c`, `9594e42`, **detached et verrouillés** — seul reste à nettoyer |

Les 13 commits non publiés : la clôture du round 1 de `b-derivation` et les
deux décisions `BDER-006`/`BDER-008` (`513b366`) ; le lot balises — la pré-gate
accepte une ligne de tags, renommage `G4` → `g4-client-surfaces`, pré-gate
câblée dans la CI (`bfab39e`, `2d89543`, `9594e42`) ; le round 2 complet de
`b-derivation` — correction, rapport, gel de Pass A, `BDER-006` et `BDER-008`
`VERIFIED`, `BDER-013` ouvert, revue d'impact (`4f5921e` → `804e7bb`) ; et le
merge (`72b96ec`).

**Précondition du fil** : il clone `origin`. Tant que ces 13 commits ne sont
pas poussés, un run partirait de `db01690` et referait à l'aveugle un travail
déjà fait. Le garde-fou est inscrit dans `AM-4` § *Prohibitions* : le fil
refuse de tourner sur une base `main` qu'il n'a pas lui-même vérifiée.

---

## 2. `codex/audit-c-headers` — le round de juillet

Il n'était pas perdu. Branche locale `af32734`, jamais poussée, jamais
fusionnée, basée sur `240c658`. Son contenu est substantiel : **2158 lignes
ajoutées, 15 fichiers**.

| Fichier | Lignes |
|---|---|
| `docs/audits/features/c-headers.md` | 639 — audit public, findings `CHDR-001` … `CHDR-016` |
| `auditor/runs/2026-07-30-audit-initial.md` | 438 |
| `auditor/runs/pass-a/RU-1…RU-4.md` | 148 + 147 + 152 + 153 — les quatre unités de revue, gelées séparément |
| `.agents/c-headers/DOMAIN.md` | 183 |
| `.agents/c-headers/STATE.md` | 111 |
| skills auditeur + correcteur, `agents/openai.yaml` | 142 |
| `features/c-headers.feature` | +37 (marqueurs d'audit) |
| `features/AGENTS.md` | +7 (routage) |

Un round manuel abouti, avec plusieurs `CHDR` en P1/P2 — précisément ce qu'il
faut comme étalon.

### Pourquoi il ne peut pas être repris tel quel

La branche part de `240c658`. Le correctif `BDER-011` est arrivé **après**
(`78c06ba`, `090d11a`, tous deux dans `240c658..main`). Ce round a donc
collecté ses preuves de gate sur un harnais qui **ne pouvait pas échouer** :
`filter_run` sous `harness = false` renvoyait 0 même avec des scénarios en
échec. Les définitions de steps ont bougé depuis.

`PROCESS.md` tranche seul : on ne rebase pas silencieusement des preuves déjà
collectées, et on ouvre un nouveau round dès que le comportement concerné a pu
changer.

**Décision (2026-08-03) : nouveau round depuis le `main` courant.** `af32734`
est poussée et conservée, mais n'entre **jamais** en Pass A — uniquement en
Pass B, et comme point de comparaison du jalon de vérité : le fil retrouve-t-il
les `CHDR` P1/P2 du round manuel ?

---

## 3. Bootstrap conteneur : prouvé

| Contrôle | Résultat |
|---|---|
| Clone `aithos-protocol/aithos-core` | 20 s, anonyme (dépôt public) |
| `cargo` | 1.95.0 |
| **`cargo metadata` sans checkout frère `aithos-client`** | **OK — 5 paquets.** L'entrée morte de `rust/Cargo.toml:111` ne bloque pas. Le fil n'a pas besoin du frère. |
| `verify-feature-tags.sh` @ `db01690` | **rouge** — `gateway-delegated-client-surfaces.feature`. C'est exactement ce que corrige le lot balises non poussé : confirmation indépendante du retard d'`origin`. |
| Gate `@a-identity` @ `db01690` | **vert** — `exit=0`, `1 feature · 8 rules · 30 scenarios (30 passed) · 93 steps (93 passed)`, **2 min 38 à froid** |
| Transcript | `evidence/ev-a-identity-db01690.txt`, sha256 `ef5d5a01e0ee19e900d21383636d9acdd632f5d472a4f6451888fc1e2df4886c` |

Deux enseignements chiffrés :

- **facteur `Scenario Outline` ×2,3** — 13 déclarations dans le fichier, 30
  scénarios exécutés. Tous les dimensionnements du chantier comptent des
  déclarations : la charge réelle des unités Pass A est plus élevée ;
- **2 min 38 à froid**, l'essentiel étant la compilation, avec un
  `CARGO_TARGET_DIR` partagé. Les gates suivants sur la même révision sont
  quasi gratuits : l'exécution centralisée des gates (`AM-1`) paie deux fois,
  en falsifiabilité et en temps.

Le contrôle croisé `exit code ↔ compteurs [Summary]` fonctionne sur du réel.
C'est la sonde permanente contre un retour du bug `BDER-011` — dont la §2
vient de montrer qu'il n'est pas théorique.

---

## 4. Amendements `AM-1` à `AM-5`

Livrés : `PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md` (`PROCESS.md`
amendé, 371 → 520 lignes) et le diff unifié
`PROPOSITION-PROCESS-AM-1-5-2026-08-03.patch` (151 lignes ajoutées, aucune
supprimée). **`features/.agents/PROCESS.md` n'est pas modifié** tant que les
amendements ne sont pas validés.

| # | Section | Ce qui est ajouté |
|---|---|---|
| AM-1 | `Feature targeting and gate pyramid` → *Orchestrated gate execution* | l'orchestrateur exécute, le rôle possède et cite un `evidence_id` ; un rapport citant une commande absente du journal est invalide ; contrôle croisé exit ↔ `[Summary]` |
| AM-2 | `Review-unit isolation` → *Material isolation of Pass A* | extrait sans `.git` pour Pass A et pour le Pass A du reviewer ; une consigne de ne pas lire l'histoire ne suffit pas pour un agent non surveillé |
| AM-3 | nouvelle section *Adversarial refutation* | panel obligatoire sur P1/P2, réfutation par défaut si incertain, majorité contre l'auditeur = blocage |
| AM-4 | nouvelle section *Orchestrated mode* | absence de mémoire · frontmatter `STATE.md` · `QUEUE.yaml` / `ledger.jsonl` / `BLOCKED.md` · transitions gardées · gardien de process · **liste close** des 10 blocages · barrière de divulgation · interdits |
| AM-5 | `Impact review` | « la décision de rouvrir reste manuelle » vaut aussi en mode orchestré (`FULL_AUDIT` = blocage) ; l'interdiction de lancer un agent lie le *rôle*, pas l'orchestrateur |

Deux formulations engagent le process au-delà de l'outillage :

- **la barrière de divulgation** (`AM-4`). Les branches étant poussées sur le
  dépôt public, le filtre ne peut plus être « ne publie pas » mais « n'écris
  pas » : un finding dont l'énoncé décrirait une faiblesse exploitable avant
  correctif ne doit atterrir dans aucun fichier suivi ; l'agent consigne un
  identifiant et un titre neutre, et bloque ;
- **« une feature close sans aucun finding est un cas à examiner, non un
  succès »** (§ *Process warden*). Seule phrase du patch qui juge la qualité
  plutôt que la forme, parce que le mode de défaillance le plus probable d'un
  fil autonome n'est pas l'erreur mais le vide bien formaté.

---

## 5. Décisions prises le 2026-08-03

| # | Sujet | Décision |
|---|---|---|
| 1 | Runtime | session Cowork cloud, conteneur éphémère |
| 2 | Autonomie | le fil avance seul jusqu'au blocage ; aucun accord humain entre deux agents |
| 3 | Déclenchement | manuel, puis zéro intervention |
| 4 | Durabilité | branches poussées sur `aithos-core` **public** → la barrière de divulgation devient une barrière d'**écriture** |
| 5 | Round c-headers | **refaire** depuis le `main` courant ; `af32734` poussée et conservée comme étalon, jamais en Pass A |
| 6 | `codex/bundle-publication-performance` | poussée pour ne pas la perdre, traitée hors du fil |
| 7 | Documents de conception | commités ; `PROCESS.md` **non modifié** tant qu'`AM-1`→`AM-5` ne sont pas validés |

Restent ouvertes : la validation d'`AM-1`→`AM-5` et de la barrière de
divulgation telle que rédigée ; le nombre de réfuteurs (2 ou 3) ; le périmètre
`aithos-service`.

---

## 6. Ce qu'il reste à faire, et par qui

Aucune commande `git` écrivant dans ce dépôt ne peut venir du pont device.
**Ces gestes sont ceux de Mathieu**, dans son terminal :

```bash
cd /Volumes/Math17/aithos/v2/code/aithos-core

# 1 — worktrees fantômes (verrouillés)
git worktree unlock /tmp/wt-b && git worktree remove --force /tmp/wt-b
git worktree unlock /tmp/wt-c && git worktree remove --force /tmp/wt-c
git worktree prune -v
git worktree list                    # ne doit rester que le dépôt principal

# 2 — documents de conception (PROCESS.md non touché)
git add docs/CHANTIER-ORCHESTRATEUR-AUDIT-GHERKIN-2026-08-03.md \
        docs/RECONNAISSANCE-ORCHESTRATEUR-2026-08-03.md \
        docs/PROPOSITION-PROCESS-AMENDE-AM-1-5-2026-08-03.md \
        docs/PROPOSITION-PROCESS-AM-1-5-2026-08-03.patch
git commit -m "docs(agents): chantier orchestrateur d'audit Gherkin — conception, reconnaissance, proposition d'amendement PROCESS (AM-1..AM-5, non appliquée)"

# 3 — publier
git push origin main
git push origin codex/audit-c-headers
git push origin codex/bundle-publication-performance

# 4 — vérifier
git rev-list --left-right --count origin/main...main   # attendu : 0  0
git branch -vv
git status --short
```

Ensuite seulement : la couche d'état (`QUEUE.yaml`, frontmatter des `STATE.md`,
format du ledger), puis le premier cycle du fil sur `c-headers`, dont le round
de juillet fournit l'étalon.
