# ADDENDUM au HANDOFF P5 DONE — la première racine quotidienne RÉELLE (2026-07-21, 06:50 Paris)

> **ARCHIVE DE PREUVE.** Observation live datée, conservée pour audit ; elle ne
> constitue pas un état de santé actuel.

À lire comme le post-scriptum de HANDOFF-PROVIDER-P5-WITNESS-DONE-2026-07-20.md §3
(« ce que le gate n'a PAS prouvé : rollover/racine réels en prod ») : ce point est CLOS.

Constat du matin (wire public seul, vérification indépendante PyNaCl+blake3 depuis
un container NEUF — aucun état de session, aucun code du dépôt) :

| Preuve | Résultat |
|---|---|
| `witness.aithos.fr/roots/2026-07-20.json` | **200 — la racine du premier jour réel EXISTE** (scellée par le balayage D1 au passage de minuit UTC, sans intervention) |
| Signature de la racine | VERTE sous la clé du registre `keys.json` (lui-même auto-vérifié) |
| `root` recalculé depuis le feed public | **byte-exact** : mroot left-heavy, domaines `aithos-witness/v1/mk-leaf|mk-node`, lignes du jour triées/dédupliquées → `21d9f978c12978eb…` == publié, `n = 2` == compté |
| Store / witness / feed | healthz 200 ; keys.json 200 (max-age=60) ; le feed du DID de rejeu sert toujours ses 2 lignes (append-only C.3, design D8) |

Le service a donc traversé la nuit, observé le rollover, scellé la racine — le
correctif D1 (verdict témoin, corrigé avant clôture) est prouvé EN PROD, pas
seulement au harnais. Restent non exercés (inchangés) : rotation de clé,
équivocation en prod, reconcile au restart avec heads non vide, charge multi-DID.
