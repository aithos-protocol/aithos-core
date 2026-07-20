# PROPOSITION — Gate étape 6 (P2 store réel) : compute du store + tenant de rejeu

> **Statut : ACTÉE, 2026-07-20 — GO Mathieu en session sur les deux décisions :
> ① Fargate, ② retrait + tenant jetable. Notes gravées le même jour dans
> INFRA-PROVIDER §7 et §8.** Exécutait la note gravée d'INFRA-PROVIDER §7
> (2026-07-18, gate M2) :
> « compute du store : Lambda vs Fargate, à trancher au gate P2 » — et le point
> d'arbitrage n°6 de HANDOFF-PROVIDER-AWS (2026-07-17) : « tenant de rejeu `acme`
> à clés publiques, à trancher au gate P2 ». Une fois arbitré : gravure en note
> §7 (décision ①) et note §8/P7 (décision ②), même rituel que les redlines gate 4/5.

## Décision ① — Compute du store : **rester Fargate** (recommandé)

### L'argument qui tranche seul : le wire gravé est incompatible avec Lambda

A.8 grave `objet ≤ 32 MiB` en PUT **direct** (le corps DANS la requête, c'est ce
que l'enveloppe signe via `body_b3`) et `batch ≤ 32 MiB de réponse` en
`multipart/mixed`. Or, chez Lambda (vérifié en ligne 2026-07-20, docs AWS) :

| Chemin d'entrée | Requête max | Réponse max |
|---|---|---|
| ALB → Lambda (notre front actuel) | **1 Mo** | 1 Mo |
| Function URL / invoke sync | **6 Mo** | 6 Mo (buffered) |
| Function URL + response streaming | **6 Mo (la requête ne streame pas)** | 200 Mo (depuis 07/2025) |

Le streaming ne résout que le sens sortant. Un PUT de blob de 32 MiB **ne peut
physiquement pas atteindre** un handler Lambda sans rupture du wire : il faudrait
soit une redline A.8 (borne à ~6 MiB — régression produit sans cause), soit un
détour presigned-URL S3 (le corps quitterait l'enveloppe signée — rupture de
`body_b3`, redésign complet d'A.2, exclu). Fail-closed : le wire prime, le
compute s'y plie.

### Arguments de renfort (aucun n'est nécessaire, tous convergent)

- **Latence.** Cible §3.6 : append p50 < 120 ms depuis l'Europe. Le chemin chaud
  étape 6 = vérif Ed25519 + nonce DynamoDB + écriture conditionnelle têtes +
  PUT S3 : le budget est déjà serré ; « toujours chaud » (doctrine §7 d'origine)
  reste la position sûre. Les ~15–30 ms de cold start Rust sont modestes mais
  s'ajoutent précisément au premier hit — celui que la démo montre.
- **Infra existante prouvée.** ALB + Fargate + ECR + DynamoDB est déployé,
  gate P1 validé contre la prod, plans Terraform à 0 écart. Migrer sur Lambda =
  réécrire `modules/store-api` (ALB→Function URL/API GW), re-prouver le gate
  déployé, pour un service qui marche.
- **Coût non décisif.** Le poste Fargate du store (0.25 vCPU / 0.5 Go,
  eu-west-3) ≈ 10–15 €/mois — déjà dans le budget MVP « dizaines d'€/mois »
  (§7). L'économie scale-to-zero de Lambda est du même ordre : elle n'achète
  rien qui vaille une rupture de wire.
- **HA.** Le préalable commun aux deux options (sortir l'état de la mémoire par
  tâche → S3 + DynamoDB) est exactement le contenu de l'étape 6 ; une fois fait,
  `desired_count = 2` derrière l'ALB donne la cible 99,9 % sur Fargate sans
  autre travail.

### Ce que la décision ne ferme pas

Conforme à la note §7 : le **témoin** et le **plan public** (feed, racine
quotidienne, éventuel rendu CloudFront) pourront passer serverless plus tard
sans toucher les wires — leurs payloads sont minuscules et sortants. Le
**relais reste Fargate/NLB quoi qu'il arrive** (note §7, non rouvert ici).

### Conséquence immédiate pour l'étape 6 (si GO)

Aucun changement de front : le code étape 6 remplit les seams existants
(`ObjectStore.list` → ListObjectsV2 ; CAS A.5 → écritures conditionnelles
DynamoDB coordonnées au dépôt S3 ; write-once ⑧b ; classes de cache A.6), la
task def Fargate gagne S3 + table des têtes dans son rôle (moindre privilège,
pattern nonces), et `desired_count` passe à 2 **au gate déployé seulement**.

## Décision ② — Tenant de rejeu `acme` (clés PUBLIQUES) : **retrait de l'image prod** (recommandé)

Rappel du risque (point n°6, 17/07) : le bootstrap embarqué de l'image prod
contient le tenant `acme` ancré sur les clés des vecteurs committés — publiques
par construction. Sans risque tant que le store est en mémoire (rien ne
persiste) ; **dès que l'étape 6 branche S3, n'importe qui possédant le dépôt
peut écrire des objets durables dans la prod.** Ça se règle à ce gate, avant le
premier apply étape 6.

Options :

- **(a) Recommandée — retrait + tenant de rejeu jetable.** Le bootstrap embarqué
  disparaît de l'image prod ; le control plane (table DynamoDB P7, déjà
  déployée) devient la seule source de tenants. Le rejeu déployé du gate se fait
  sur un tenant `replay-<date>` créé par la CLI d'admin (P7) juste avant, purgé
  juste après (purge outillée = la même mécanique GC que §8 exigera de toute
  façon). Preuve du gate inchangée, zéro matériel public persistant.
- **(b) Env `dev` séparé.** Retour d'un `envs/dev` complet portant le tenant de
  rejeu. Propre mais re-crée un environnement entier pour un besoin ponctuel —
  contraire à l'arbitrage « plateforme unique » du 17/07.
- **(c) Garder `acme` avec quota zéro écriture.** Rejeu en lecture seule —
  mais le gate étape 6 doit précisément prouver des ÉCRITURES durables. Insuffisant.

## Ce que ce dossier ne décide pas

La forme exacte de la coordination S3↔DynamoDB du CAS (transaction, ordre des
effets, reprise sur panne à mi-écriture) : c'est du design d'implémentation
étape 6, il arrive feature-first au prochain gate contrat, pas ici. Idem pour
la matérialisation CloudFront d'A.6 (module `cdn-public`).
