# Démo « Léa » — un agent commercial borné, briefé et audité

> **Statut : SCÉNARIO VALIDÉ ET AUTOMATISÉ.** Ce document est la
> référence : les quatre contrats Gherkin le traduisent, les huit beats
> passent en e2e local et provider, et rien ne se code hors des contrats.
> Runbook provider courant : `DEMO-LEA-PROVIDER-CLI.md`.

## 1. Le pitch

Innoestate (startup proptech) confie la prise de rendez-vous prospects à
une agente IA, **Léa**. L'entreprise ne « fait pas confiance » à Léa : elle
la **mandate**. Trois connecteurs — Notion (lecture seule), Gmail
(écriture bornée), Calendar (créneaux bornés) — les tokens au coffre,
chaque acte et chaque refus signés dans le gamma, et un « caractère »
(consignes public/circle) servi par le hub lui-même, indépendant du
fournisseur de LLM.

**Clarification structurante (Mathieu, 2026-07-15)** : les trois
connecteurs sont des serveurs MCP **séparés** — chacun son endpoint,
chacun son token **pleine puissance** dans le coffre. L'amont ne bride
rien : si un appel interdit franchissait le gateway, Gmail l'exécuterait.
Le seul point de passage de Léa est l'endpoint unique du gateway, et
**c'est lui — grants, bornes, mandats — qui restreint**. Les scénarios
« zéro hit amont » prouvent précisément que le mur est le gateway, jamais
le connecteur.

Un prestataire externe (Mathieu) pilote Léa : il n'a **aucun** privilège
protocolaire — les bornes sont côté agent, qui parle ne change rien. Son
seul vrai pouvoir : le **mandat d'auditeur**, pour rejouer toute l'histoire
à la fin.

## 2. Décisions actées (2026-07-15)

1. **Refus pédagogique** : un refus de borne révèle le champ, les valeurs
   fautives ET la règle approuvée. Ce n'est pas une fuite : c'est
   exactement le périmètre que l'owner a granté (déjà scellé et loggé).
   On ne se contente pas de borner, on explique.
2. **Bornes v1** : quatre types, déterministes — `one_of` (whitelist de
   valeurs ; couvre destinataires ET sous-actions d'un outil polymorphe),
   `time_slots` (jours + plage horaire), `forbid`/`require` (présence),
   `max_items` (taille de tableau). Le reste est hors v1 (§7).
3. **Un seul agent** (Léa) ; le prestataire externe est narratif + mandat
   d'auditeur réel. Pas d'authentification du côté agent du endpoint en v1.
4. **Notion en lecture seule** ajouté : la liste des prospects vient de la
   donnée (5 noms), le mandat n'en autorise que 3 — la donnée propose, le
   mandat dispose.
5. **Briefing conditionnel** : des consignes existent dans les zones
   grantées → l'outil `briefing.read` apparaît et `initialize` le
   recommande ; tout est vide et rien n'est inscriptible → surface muette.
   La zone `self` ne parvient **jamais** à l'agent.
6. **Le gateway ne réécrit jamais** un appel : il refuse en entier et
   explique. Retirer silencieusement les destinataires interdits
   trahirait l'agent et salirait l'audit.

## 3. Distribution et provisioning

| Serveur (mock BDD / réel jour J) | Outil amont | Classe | Décision | Bornes |
|---|---|---|---|---|
| notion | `query_database` | read | **grantée** | — |
| notion | `create_page` | write | refusée | — |
| gmail | `search_emails` | read | **grantée** | — |
| gmail | `send_email` | write | **grantée** | `to` one_of {prospect-a, prospect-b, prospect-c} ; `bcc` forbid ; `to` max_items 3 ; `subject` require |
| gmail | `delete_email` | write | refusée | — |
| calendar | `list_events` | read | **grantée** | — |
| calendar | `create_event` | write | **grantée** | `start` time_slots {mar, jeu} 14:00–18:00 |

- **Ethos « ventes »** unique : les trois serveurs y sont enrollés, un seul
  mandat agent couvre les outils grantés (+ journal + xref, comme
  toujours).
- **Coffre** : un token par serveur dans Vault KV v2
  (`aithos/mcp/{notion,gmail,calendar}`), YAML = références (tranche déjà
  verte).
- **Consignes** (zone circle de « ventes ») : « Tout mail de prise de
  rendez-vous mentionne le DPE du bien et propose d'abord une visite
  virtuelle. » Une section `self` existe aussi (notes owner) — pour
  prouver qu'elle ne sort jamais.
- Prospects en base Notion (mock) : a, b, c, d, e — la whitelist n'en
  couvre que trois.
- **Amont plein pouvoirs, toujours** : les mocks BDD acceptent TOUT appel
  qui les atteint (et le jour J, les tokens réels porteront les scopes
  maximaux). Aucune restriction ne vient de l'amont — la démonstration
  n'a de valeur que comme ça.

## 4. Le storyboard (8 beats)

1. **Surface exacte.** Léa s'initialise : `initialize.instructions` signale
   les consignes et recommande `briefing.read` avant toute action
   sortante. `tools/list` : exactement les outils grantés + `briefing.read`
   + `journal.*` ; `gmail__delete_email` et `notion__create_page`
   invisibles. *Prouve : exposition = couverture du mandat.*
2. **La donnée vient de Notion.** `notion__query_database` → 5 prospects.
   Acte loggé, le mock Notion voit SON bearer sorti du coffre. *Prouve :
   multi-connecteurs sous un Ethos, read-only réel.*
3. **Le mur qui enseigne.** Envoi aux 5 → refus : « `send_email.to` :
   prospect-d, prospect-e hors de la liste approuvée {a, b, c} ». Refus
   loggé (contexte + journal), **zéro hit coffre, zéro hit Gmail**.
   *Prouve : bornes d'arguments, fail-closed, refus pédagogique.*
4. **L'auto-correction.** Envoi à a, b, c → passe. Gmail voit UN appel,
   bearer du coffre, nom brut. Acte + xref. *Prouve : l'agent se corrige
   seul grâce au refus.*
5. **Les créneaux.** `create_event` mercredi 10:00 → refus nommant
   {mar, jeu 14:00–18:00} ; jeudi 15:00 → passe. *Prouve : time_slots.*
6. **Le caractère.** `briefing.read` → la consigne DPE/visite virtuelle,
   lecture journalisée ; les mails la respectent (jour J, visible à
   l'écran). *Prouve : couche molle gouvernée, portable.*
7. **Édition à chaud.** L'owner modifie la section circle (« ajouter le
   lien du dossier de visite ») → le `briefing.read` suivant sert le
   nouveau texte, sans redémarrage. *Prouve : caractère sans fournisseur.*
8. **La preuve.** Mandat d'auditeur : `audit-export` du contexte ventes —
   actes Notion/Gmail/Calendar, les deux refus avec leurs raisons, les
   lectures de briefing, tout signé et chaîné. Grep des tokens : zéro
   occurrence hors coffre et fil. *(Bonus si le rythme le permet :
   rotation du token Gmail dans Vault en live.)*

## 5. Les lots (BDD-first, dans l'ordre)

### Lot W — octroi des writes (`gateway-grants.feature`)

Séparer **classe de risque** (read|write — la nature du pouvoir) et
**décision d'octroi** (granted|denied — ce que CET agent peut faire).
Défauts conservateurs = sémantique historique : une approbation qui ne
nomme qu'une classe grante les reads et refuse les writes. Un write granté
entre dans `tools/list`, son mandat le couvre, ses actes se loggent comme
les autres ; un outil non granté (toute classe) reste caché et refusé
nommément. Changer une décision = re-enrollment (nouveau mandat,
révocation politique de l'ancien — le kill switch de la démo). Config et
manifeste doivent s'accorder sur la décision, fail-closed.

### Lot P — bornes d'arguments (`gateway-bounds.feature`)

- **Où** : dans le manifeste approuvé, scellé dans le vault `/x/<server>`
  de l'Ethos — jamais dans le YAML runtime. Changer une borne = re-enroll.
  Les bornes n'entrent PAS dans le `pin_sha256` (qui fige la parole de
  l'AMONT ; les bornes sont la politique de l'owner — le scellement du
  manifeste porte leur intégrité).
- **Quand** : resolve → pin → authorize (mandat) → **bornes** → log →
  relais. Violation = refus `bound_violated` loggé à la place de l'acte,
  zéro hit coffre/amont.
- **Sémantique fail-closed** : sur un tableau, CHAQUE élément doit passer
  (un intrus refuse tout l'appel) ; type inattendu = refus ; champ absent
  sous `require` = refus ; `one_of` sur champ optionnel absent = passe ;
  champs adressés à la racine des arguments (pas de nesting en v1).
- **`time_slots`** : évalués sur l'heure locale ÉNONCÉE dans l'argument
  (RFC 3339) — une visite à 15 h est à 15 h heure du bien ; pas de base
  de fuseaux embarquée en v1 (documenté).
- **Refus pédagogique** : champ + valeurs fautives + règle approuvée
  (décision 2026-07-15 n°1).
- Bornes sur un outil non granté = rejet à l'approbation.

### Lot K — briefing (`gateway-briefing.feature`)

Outil natif `briefing.read` (préfixe `briefing` réservé comme `journal` et
`gateway`), servi par le gateway depuis les zones **public + circle** des
Ethos grantés — `self` jamais. Contenu exact de l'owner, étiqueté par
contexte ; chaque lecture journalisée (entrée de lecture dans le gamma du
contexte). Surface conditionnelle (décision n°5) : consignes présentes →
outil listé + `initialize.instructions` ; tout vide et rien
d'inscriptible → ni outil ni instructions. Édition owner → la lecture
suivante sert le nouveau texte, sans redémarrage. Outillage owner : une
commande pour écrire/mettre à jour les sections de consignes (réutilise la
mécanique sections du core via `core_bridge`, seule porte).

### Lot D — la répétition générale (`gateway-demo-lea.feature` + e2e)

Le storyboard §4 en Gherkin, trois MCP mockés + faux Vault, **zéro LLM**
(le harness envoie les mêmes JSON-RPC que l'agent réel enverra). Puis un
e2e réseau binaire réel façon `e2e_vault` et, en clôture, le runbook
`DEMO-LEA.md` (connecteurs réels : Notion MCP HTTP direct ; Gmail/Calendar
via wrapper HTTP loopback — état de l'offre MCP Google à vérifier à ce
moment-là ; Cowork branché sur l'endpoint unique).

## 6. Gates

1. Validation de CE document + des quatre contrats (avant toute impl).
2. Suite verte lot par lot (tests + clippy + fmt), commits sélectifs
   `feat/obligations`, synchro Mac sha-croisée, handoff par session.
3. Répétition générale avec Mathieu en conditions réelles avant le jour J.

## 7. Hors v1 (explicitement)

Regex et suffixes de domaine (`*@innoestate.fr`), règles croisées entre
champs, bornes sur champs imbriqués, règles de CONTENU (« mentionner le
DPE » est le travail du briefing, pas d'un mur dur), `resources/*` MCP,
authentification du endpoint agent, wrapper stdio générique (Phase D),
second agent « Relation clients » (Notion write) — envisageable ensuite
sur les mêmes briques.
