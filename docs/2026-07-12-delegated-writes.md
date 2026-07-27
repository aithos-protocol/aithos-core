# Écritures déléguées (circle) — verdict, preuve et correctif

> **ARCHIVE DE DIAGNOSTIC.** Le correctif fait partie de l'historique du Core ;
> les chemins et commandes de cette note ne décrivent plus le worktree courant.

> 2026-07-12. Réponse à l'objection « on ne peut pas écrire une section avec
> un mandat sur self ou circle » soulevée pendant le développement de la
> gateway. Verdict : **faux au niveau du protocole, vrai au niveau de
> l'implémentation d'alors** — corrigé par cette passe (pass L), circle
> d'abord.

## 1. La preuve côté protocole (la spec l'a toujours permis)

1. **§04.2 — la grammaire du périmètre** : `verb := read | edit | append |
   delete | write` sur `zone := public | circle | self`, avec la lattice
   normative *« read ⊑ edit ⊑ append ⊑ write ; every mutation verb implies
   read »* et *« append = create + edit within perimeter ; write = full
   CRUD »*. Quatre des cinq verbes n'auraient aucun sens sans écriture
   déléguée.
2. **§07.2 — qui signe une entrée** : *« **Delegated** entries
   (**mutations** or actions by an agent): signed by the leaf grantee
   key… »*. Le log nomme explicitement les mutations d'agent.
3. **§04.3 — la physique** : la header line livrée au grantee porte la clé
   de nœud, qui est **symétrique** : qui peut ouvrir peut sceller.
   `agent_node_key()` existait déjà ; il ne manquait que la surface d'API.
4. **§04 en-tête** : un mandat autorise à *« read/author a perimeter »*.

Seule restriction réelle (§04.2, §02.8) : sur `self`, les périmètres
d'écriture `dir=`/`tag=` sont read-only (structure scellée) ; l'écriture
self passe par `id=` ou le grant de zone entière. Sur `circle` : aucune.

## 2. Ce qui manquait dans le code (l'objection était légitime)

- `GrantSpec` n'avait pas de champ verbe : `grant()`/`delegate()`
  émettaient `Verb::Read` en dur — impossible de MINTER un périmètre
  d'écriture.
- `bundle.rs` : `section_add/rewrite/delete` exigeaient `owner: &OwnerKeys`.
- `log.rs` : appends délégués pour `action`/`inference`/`ethos.read`/`grant`
  seulement — aucun logger de mutation déléguée.
- Gateway `core_bridge.rs` : aucune surface d'écriture (d'où l'impression).
- Zéro occurrence de `edit.circle|write.circle|…` dans features/ et tests.

## 3. Le correctif (pass L, tests d'abord — rituel respecté)

`features/l-delegated-writes.feature` écrit et exécuté AVANT le code
(12 scénarios, tous skipped au premier run), puis :

- **`aithos-core/mandate.rs`** : `Verb::parse`/`as_str` deviennent publics.
- **`aithos-bundle/grants.rs`** : `GrantSpec.verb` (toute la lattice §04.2 ;
  la clé livrée est la même, le CERTIFICAT sépare lecteur et rédacteur) ;
  `deliver_zone_line()` (livraison de line sans certificat, pour les
  mandats riches assemblés à la main) ; `agent_current_section_key()`
  (version gouvernante = plus profond header ancêtre, atteinte via lines +
  wraps de l'agent — un stylo périmé échoue, fail-closed) ;
  `section_add_as_agent` / `section_rewrite_as_agent` /
  `section_delete_as_agent` — chaîne vérifiée révocations comprises au
  `now` injecté, `covers_op` sur le verbe exigé (add→append, rewrite→edit,
  delete→delete), blob **non signé** (§02.11 : la preuve d'auteur est
  l'entrée gamma déléguée, signée par la clé grantee sous sa chaîne).
- **`aithos-bundle/log.rs`** : `log_delegated_mutation()` — jumeau scellé
  de `log_owner_mutation`, body sous la clé du nœud cible (que l'agent
  détient nécessairement — moitié physique), `verify_delegated_entry` à
  l'append.
- **`aithos-cli`** : `grant --verb read|edit|append|delete|write`.
- **`aithos-gateway/core_bridge.rs`** : `record_section_add` /
  `record_section_rewrite` / `record_section_delete` sous la chaîne agent ;
  refus mappés `MandateDenied` (périmètre/fenêtre/révocation) vs
  `LogAppendRefused`. **Suivi restant côté gateway** : le tool-map
  d'onboarding ne mint encore que des périmètres `act.*` — brancher des
  périmètres d'écriture dans la config/onboarding est la suite naturelle,
  avec ses scénarios gateway propres.

## 4. Ce que les nouveaux scénarios prouvent (203/203 verts)

- Un grant `append`/`edit`/`write` écrit/réécrit/supprime dans son dossier ;
  l'owner relit ; la dernière entrée gamma est un `section.*` délégué,
  body scellé, cible scellée (§07.3) ; un sans-clé n'apprend rien.
- La lattice est opposée : read ne réécrit pas, edit ne crée pas, append ne
  supprime pas, un rédacteur ne sort pas de son dossier, un mandat expiré
  n'écrit rien et le log ne bouge pas.
- **Super-mandat** : UN certificat portant `write.circle#dir` +
  `act.x.gmail.reply` + `read.gamma#kind=action` + `issue#depth=1` +
  `revoke` + `max_actions 2` — lit, écrit, agit, audite son log, délègue,
  révoque son délégué, épuise son budget… puis **meurt entier au jour 31**
  pendant que la clé owner écrit toujours. C'est la formalisation exacte de
  l'invariant : *un mandat fait tout ce que fait la clé owner **sur son
  périmètre** — délégation comprise — mais jamais les pouvoirs de niveau
  identité (heartbeat, élargissement, profondeur infinie, racine), et
  toujours sous fenêtre et révocation.*
