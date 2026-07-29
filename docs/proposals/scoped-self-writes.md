# Proposition de redline — écritures déléguées scopées dans la zone `self`

| Champ | Valeur |
|---|---|
| Statut | **Proposé — non adopté** |
| Date | 28 juillet 2026 |
| Portée | `self`, mandats délégués, descripteurs et operation-facts |
| Décideur | à désigner |
| Implémentation autorisée | non |

Objet : étudier `dir=` et `tag=` en **écriture** sur `self`, afin que la
délégation puisse y avoir une granularité comparable à `public` et `circle`.
Le défaut actuel susceptible de produire des lignes d'index orphelines est
documenté séparément dans
[`../audits/protocol/delegated-self-orphan-index.md`](../audits/protocol/delegated-self-orphan-index.md).

> **Révision.** Une première version de ce document justifiait la levée de l'interdiction
> par l'argument « la physique borne déjà le placement ». **Cet argument est vrai dans le
> modèle de la spec et faux dans l'implémentation actuelle.** La vérification du code a
> montré que la mutation du descripteur parent est *best-effort* et que la création
> déléguée est ancrée à la racine. La levée devient donc l'**étape 4** d'un lot de
> quatre, conditionnée par les trois premières — et non l'inverse.

---

## 1. Ce qui existe déjà — la parité n'est pas le problème

§4.2 est explicite : *« `self` writes use `id=` **or zone-level grants** »*.

Normativement, un délégué porteur de `write.self`, `append.self`, `edit.self`
ou `delete.self` **sans sélecteur** reçoit une autorité sur toute la zone. Les
scénarios de `features/l-delegated-writes.feature` couvrent cette forme, mais
leur seule réussite ne constitue pas une preuve sémantique tant que leurs steps
n'ont pas été audités.

La proposition part donc d'une parité normative au niveau zone ou `id=` et
cherche à ajouter de la **granularité**.

| Autorité | Verdict actuel |
|---|---|
| `append.self` (zone entière) | accepté |
| `append.self#id=<sid préalloué>` | accepté |
| `edit.self#id=<sid>` · `delete.self#id=<sid>` | accepté |
| **`edit.self#dir=<dossier>`** | **refusé** |
| **`delete.self#tag=<tag>`** | **refusé** |

---

## 2. Les trois couches de la limite

La limite n'a pas une cause mais trois, empilées, et de natures différentes. Les
distinguer est ce qui rend la décision possible.

| Couche | Nature | Révisable ? |
|---|---|---|
| **1. La structure de `self` est confidentielle** (§2.8) | Choix de design, propriété produit | Non — la toucher rend l'arborescence privée lisible par le provider |
| **2. Un vérifieur sans clé ne peut pas contrôler la couverture `dir=`** | Conséquence technique réelle de la couche 1 | Non sans preuve à divulgation nulle |
| **3. L'interdiction de `dir=`/`tag=` en écriture** | Politique prudente adoptée face à la couche 2 | **Oui — c'est l'objet de cette redline** |

L'interdiction n'est imposée ni par 1 ni par 2. C'est une décision de repli, et c'est
elle seule que ce document propose de lever.

---

## 3. Précondition issue de l'audit de l'implémentation

L'audit séparé confirme que `grantee_create_self` est ancré à la racine, écrit
la ligne d'index avant de savoir si le descripteur peut être mis à jour et
ignore l'échec de dérivation de la clé de zone. Le modèle où la possession de
la clé du parent borne physiquement le placement n'est donc pas encore vrai
dans le code.

La présente redline ne doit pas être appliquée avant la fermeture et la preuve
de ce défaut. Lever directement l'interdiction créerait un périmètre `dir=`
affiché dans le certificat mais garanti ni par un vérifieur sans clé, ni par la
physique de l'arbre.

---

## 4. Le lot, en quatre étapes ordonnées

### Étape 1 — rendre la mutation du descripteur obligatoire

Aucune ligne ajoutée à `e/self/index.json` sans entrée correspondante dans le
descripteur du dossier ciblé. Échec fermé si l'acteur ne peut pas l'ouvrir. Symétriquement
pour la suppression.

C'est cette étape qui fait passer la physique de *décorative* à *contraignante*, et donc
qui rend vrai l'argument central de la redline.

### Étape 2 — rendre la création déléguée capable de viser un dossier

Supprimer l'ancrage racine de `grantee_create_self`, avec la symétrie owner/délégué qui
manque aujourd'hui. Résout au passage un bug ouvert.

### Étape 3 — interdire les orphelins

Règle de vérification : **toute ligne de l'index `self` est référencée par exactement un
descripteur.**

- **Sans clé** : contrôlable en *cardinal* — le nombre de lignes de l'index est déjà
  public, et une divergence entre ce nombre et le nombre d'enfants engagés se détecte
  sans rien révéler de la structure.
- **Par détenteur de clé** : contrôlable en *placement*, par parcours des descripteurs.

Cette étape ferme un trou qui existe **déjà**, indépendamment de la redline.

### Étape 4 — lever l'interdiction

Seulement alors : rendre `dir=` et `tag=` valides comme périmètres d'écriture sur `self`,
avec l'application à trois niveaux ci-dessous.

| Niveau | Qui applique | Quoi |
|---|---|---|
| **Physique** | la cryptographie | après l'étape 1, muter exige d'ouvrir le descripteur parent, donc de détenir sa clé — écrire sous un dossier non détenu devient **impossible**, pas seulement interdit |
| **Vérifiable par détenteur de clé** | owner, titulaire d'un grant de zone, titulaire du dossier | les *parent SID arrays* déjà engagés dans le document d'operation-facts protégé (K1.2-M-B) permettent de rejouer le contrôle de couverture a posteriori |
| **Vérifiable sans clé** | tout tiers | la forme : preuve opaque bien formée, chaîne valide, entrée gamma présente, racines et édition cohérentes, cardinal de l'index conforme — c'est-à-dire ce qu'il vérifie déjà, plus l'étape 3 |

---

## 5. Rédaction proposée

**§2.8**, en remplacement de la phrase actuelle :

> Sur `self`, les périmètres `dir=` et `tag=` sont **applicables en lecture comme en
> écriture**, mais leur mode d'application diffère de `public` et `circle`. Ils sont
> appliqués par **physique** : toute mutation exige l'ouverture du descripteur scellé du
> dossier parent, donc la détention de sa clé, et aucune autorité de certificat ne peut
> suppléer cette détention. Toute ligne de l'index `self` est référencée par exactement
> un descripteur ; une divergence de cardinal est un refus, contrôlable sans clé. Le
> contrôle de couverture est en outre **rejouable par tout détenteur de clé** à partir
> des tableaux de SIDs parents engagés dans le document d'operation-facts protégé. Un
> vérifieur sans clé contrôle la forme, la chaîne, l'entrée gamma, les racines et le
> cardinal — jamais la relation de contenance, qu'il ne peut pas voir et sur laquelle il
> n'a jamais eu de garantie dans cette zone.

**§4.2**, en remplacement de la phrase actuelle :

> Sur `self`, `dir=` et `tag=` sont applicables en écriture par physique et rejouables
> par détenteur de clé (§02.8), et non contrôlables par un vérifieur sans clé — un
> partage explicite, du même ordre que celui de `read.gamma` ci-dessous.

Le précédent existe : §4.2 pratique déjà ce partage pour `read.gamma` — *« Enforcement is
split honestly : `dir`/`id`/`tag` are **physics**… `kind`/`since`/`until` are certificate
policy. »*

---

## 6. Ce qui n'est pas concédé

**La confidentialité ne bouge pas.** Accorder `write.self#dir=F` livre exactement la même
ligne de header que `read.self#dir=F`, **déjà autorisé**. Zéro clé supplémentaire, zéro
visibilité supplémentaire. Seule change la grammaire de périmètre acceptée dans un
certificat.

**L'intégrité de l'index doit rester vérifiable sans clé.** L'index `self` est
opaque mais en clair : un vérifieur peut en dériver un diff. §2.6.1 exige que
les changements soient expliqués par l'acteur et sa chaîne. Le câblage complet
de cette exigence dans les vérifieurs de production doit néanmoins être audité
et prouvé avant d'en faire une garantie de l'implémentation.

Si les préconditions de cette proposition sont satisfaites, l'angle mort
spécifique restant est le placement, dans une zone dont la structure est
secrète par décision de produit.

---

## 7. Cas limites à trancher

1. **Grant `dir=` sur un dossier `self` non encore détenu.** Le grant prospectif est
   valide (§4.2) mais **inopérant** tant que la ligne de header n'est pas livrée. À
   expliciter : le certificat seul n'ouvre rien.
2. **`tag=` sur `self`.** Les tags sont scellés dans le corps de la section, pas dans un
   descripteur — la physique y borne moins bien. Proposition : accepter `tag=` en écriture
   au niveau de la **vue de tag** (dont le wrap est scellé, donc soumis à la même physique
   que les dossiers), et traiter le retag d'une section détenue comme couvert par
   l'autorité d'édition sur cette section. À confirmer contre §2.9.
3. **Création sous un dossier `self`.** Exige `append` ou `write` (il n'existe pas de
   verbe `create`) **et** la détention de `K_dossier`. Deux conditions indépendantes, à
   vérifier séparément.
4. **Suppression d'un dossier `self`.** Exige la couverture de chaque descendant — c'est
   la preuve opaque de §2.8, dont l'encodage signé est encore **réservé**. Ce cas dépend
   de la levée de ce `FUTUR` et appartient au même lot.

---

## 8. Coût

| Lot | Contenu |
|---|---|
| **Bundle — étape 1** | `grantee_create_self` et symétriques : descripteur obligatoire, échec fermé |
| **Bundle — étape 2** | suppression de l'ancrage racine, symétrie owner/délégué sur la résolution de dossier |
| **Core — étape 3** | règle anti-orphelin ; contrôle de cardinal sans clé, contrôle de placement par détenteur de clé |
| **Spec — étape 4** | redline §2.8 et §4.2 |
| **Core — étape 4** | `covers()` accepte `dir=`/`tag=` sur `self` en écriture ; le contrôle consomme les *parent SID arrays* des operation-facts |
| **Encodage réservé** | lever le `FUTUR` de §2.8 sur la preuve opaque — prérequis du cas limite 4 |
| **Vecteurs** | inventorier les fixtures affectées, puis couvrir : écriture `dir=` autorisée ; écriture hors périmètre refusée par physique **et** par certificat ; ligne orpheline refusée ; rejeu de couverture par détenteur de clé ; vérification sans clé limitée à la forme et au cardinal |
| **BDD** | dans `features/l-delegated-writes.feature`, remplacer les deux lignes `refused` par leurs cas positifs et ajouter les cas négatifs — **écrits sans proxy et sans step fourre-tout** |

Les étapes 1 à 3 sont nécessaires indépendamment de cette redline. Leur
réalisation ne permet toutefois pas de présumer le coût de l'étape 4 : la
grammaire, les preuves opaques, les operation-facts et la compatibilité doivent
encore être chiffrés et revus.
