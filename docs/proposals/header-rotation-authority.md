# Proposition de redline — autorité du signataire d'une rotation de header

| Champ | Valeur |
|---|---|
| Statut | **Proposé — non adopté** |
| Date | 28 juillet 2026 |
| Portée | `spec/03-headers.md`, `spec/05-delegation.md`, log gamma |
| Révision inspectée | `be2d098eeb79107c861462a6433df9ef45871265` |
| Décideur | à désigner |
| Implémentation autorisée | non |

Ce document propose de fermer l'incohérence entre
`spec/05-delegation.md` §5.5 et `spec/03-headers.md`. Il ne modifie pas la
norme et ne rend pas le niveau *Core issuer* revendicable tant que la redline,
son implémentation et ses preuves n'ont pas été approuvées.

---

## 1. L'exigence est normative, son encodage reste ouvert

`spec/05-delegation.md` §5.5, lignes 97–99, est déjà normatif et sans ambiguïté :

> *« A verifier rejects a header rotation **whose signer** is not an authorized issuer for the lines it changed, or that drops a line the signer had no authority over (that would be an unauthorized revocation by omission). »*

La règle existe donc déjà. Le problème est que **`spec/03-headers.md` ne dit nulle part où ce signataire est enregistré.** Le `Header` (§3.1) est un objet `{object, v, node, key_versions{ version: {lines[]} }}` — pas de champ signataire, pas de signature, pas de référence.

§3.4 va jusqu'à énoncer une règle de vérification purement structurelle :

> *« Verification is mechanical: the new version's lines MUST equal the previous lines minus the revoked. »*

Mais « minus the revoked » suppose résolu ce que le format ne permet pas de savoir : **qui a désigné le révoqué, et en avait-il le droit.**

C'est donc une incohérence inter-chapitres à trancher par une redline, pas une fonctionnalité à inventer.

### État de l'implémentation

`aithos-core::header.rs::check_rotation` (l. 275–305) ne vérifie que deux choses :

1. aucun destinataire clandestin n'est introduit dans la nouvelle version ;
2. la ligne owner est présente (I3).

Aucun contrôle d'autorité. `vectors/g2-rotation.json` porte exactement trois assertions — `expected_survivor_kids`, `smuggled_must_fail`, `missing_owner_must_fail` — et **aucun cas d'autorité**.

### Le scénario d'attaque, concret

Le grantor G accorde à A et à B une ligne sur le même dossier `/e/circle/d/<sid>`. A et B sont des pairs : ni l'un ni l'autre n'a émis le mandat de l'autre.

A détient la DK du nœud (c'est la condition pour tourner). A tourne le nœud et republie le header **sans la ligne de B**. Le résultat passe `check_rotation` : aucun destinataire clandestin n'est ajouté, la ligne owner est là. L'édition est valide. **B est coupé, et personne ne peut établir que A n'en avait pas l'autorité.**

C'est exactement la « unauthorized revocation by omission » que §5.5 dit devoir être rejetée.

---

## 2. Un véhicule candidat existe déjà

`spec/07-gamma.md` §7.3 liste les kinds structurels à charge utile claire :

> `grant` / `revoke` / **`rotate`** / `merge` → *structural* → *clear ids/versions*

Et §7.2 fixe qui signe :

> *« **Delegated** entries … signed by the leaf grantee key …, carrying `authorized_by` = leaf id and `authorized_via` = full chain. Verified by §04.5 + §05.3 at the entry's `at`. »*

L'entrée gamma `rotate` fournit donc un candidat naturel pour porter le
signataire exigé par §5.5. Cette lecture doit être confirmée dans la redline :
la présence du kind ne définit pas encore à elle seule sa charge utile ni son
lien obligatoire avec chaque révision de header. Le kind existe aussi dans le
code, sous la variante `Kind::Rotate`.

### Ce qui manque réellement

`Kind::Rotate` est émis dans **exactement un endroit** du dépôt : `aithos-bundle::vault.rs:432`, pour la rotation d'un coffre connecteur.

**La rotation d'un nœud de contenu n'émet rien.** `aithos-bundle::revoke.rs::rotate_folder` (l. 142+) appelle `log_revoke_owner` — une entrée `revoke` qui nomme l'id du mandat cible — mais **aucune entrée `rotate` nommant le nœud, les versions et les lignes retirées.**

Conséquence à énoncer clairement : **une rotation de header est aujourd'hui une action silencieuse.** C'est-à-dire, en toute rigueur, une violation de I5 (*« Every mutation … MUST be recorded as a gamma entry naming the mandate »*) — la rotation étant une mutation sous mandat. L'incohérence est donc double, et la corriger ferme les deux d'un coup.

---

## 3. Les trois options

### Option A — l'entrée gamma `rotate` (recommandée)

Toute rotation de header émet une entrée `rotate` signée sous la chaîne du rotateur :

```jsonc
{ "kind": "rotate", "v": 2,
  "target": "/e/circle/d/<sid>",
  "payload": {
    "from_version": 3,
    "to_version": 4,
    "dropped": ["z6MkB…"],          // kids retirés, triés
    "survivors_digest": "b3:…"      // BLAKE3(JCS(kids survivants triés))
  },
  "authorized_by": "mandate_…", "authorized_via": [ … ],
  "at": "…", "prev": "…", "sig": "…" }
```

**Règle de vérification ajoutée** — pour toute révision de header `N → N+1` d'un nœud :

1. il existe **exactement une** entrée `rotate` atteignable depuis le `gamma_head` de l'édition, nommant ce nœud et ce couple de versions ; son absence invalide l'édition ;
2. `survivors_digest` correspond aux `kid` effectivement présents dans `key_versions[N+1]` ;
3. la chaîne `authorized_via` est valide à `at` (§04.5 + §05.3), non révoquée ;
4. pour **chaque** `kid` de `dropped`, le rotateur détient l'autorité au sens de I4 (règle §4 ci-dessous) ;
5. les contrôles structurels existants de `check_rotation` sont conservés inchangés.

**Encodage du header inchangé.** L'option ne requiert pas de nouveau champ
dans le header. Elle modifie toutefois le log gamma et le contenu du bundle :
l'impact sur les vecteurs byte-exact et sur les anciens vérifieurs doit être
inventorié avant de conclure à une compatibilité additive.

**Coût protocole : une entrée par rotation** — soit l'ordre de grandeur d'une révocation, opération rare et déjà coûteuse (rung 2+).

**Dépendance nouvelle** : la vérification d'une rotation requiert le log gamma.
La spécification relie déjà certains objets au gamma, mais il reste à vérifier
que toutes les surfaces de vérification disposent effectivement du journal
nécessaire.

### Option B — signer le header lui-même

Ajouter `{ "signer": …, "sig": … }` à chaque `key_version`.

**Avantage** : le header devient auto-vérifiable, sans dépendance au gamma.

**Coûts** : changement de format → bump `"v": 1` → `2` ; régénération des vecteurs C1/C2/g2/g3 ; modification de toutes les implémentations (Rust, WASM, client) ; et surtout **cela ne résout pas la partie difficile** — l'autorité sur les lignes retirées exige de toute façon la règle §4. Enfin, l'autorité serait affirmée à deux endroits (header et gamma), ce qui crée un risque de divergence entre deux sources qui doivent s'accorder.

### Option C — pointeur dans le header, signature dans le gamma

Le header porte un champ additif non signé `"rotate_ref": "<id de l'entrée gamma>"` ; la signature reste dans le gamma.

**Avantage sur A** : le vérifieur n'a pas à balayer le log pour trouver l'entrée correspondante ; le header se décrit lui-même.

**Coût** : petit changement de format additif — donc régénération des vecteurs de header quand même.

---

## 4. La règle normative à écrire (la vraie difficulté)

Les trois options partagent ce point, et c'est lui qui demande une décision humaine. « Le rotateur avait-il autorité sur la ligne retirée ? » n'est pas mécanique tant que la spec ne dit pas ce qui *justifie* une ligne.

**Formulation proposée**, à insérer en §5.5 :

> Une ligne de header adressée au `kid` K sur le nœud N est **justifiée** par tout mandat M tel que `M.grantee.pubkey == K`, que le périmètre de M couvre N, et que M est valide et non révoqué à l'instant de la rotation.
>
> Le rotateur peut retirer la ligne de K si et seulement si **tout** mandat justifiant K sur N appartient à son sous-arbre d'émission — c'est-à-dire que l'id du mandat du rotateur figure sur la chaîne de M — ou si aucun mandat ne justifie plus K (grant caduc).
>
> Le propriétaire peut toujours retirer n'importe quelle ligne : sa racine est ancêtre de toutes les chaînes.
>
> La ligne owner ne peut jamais être retirée (I3).

Cette formulation a la propriété que I4 exige explicitement : elle est **vérifiable depuis les certificats seuls**, puisque tous les certificats sont publics dans le bundle et que la résolution `kid → mandats justifiants` est une recherche sur cet ensemble.

**Trois cas limites à trancher explicitement :**

1. **Ligne orpheline** (aucun mandat valide ne la justifie — grant expiré). Proposition : retirable par tout rotateur autorisé sur N, puisqu'elle n'accorde plus rien. Alternative plus stricte : owner ou émetteur d'origine seulement. *Je recommande la première : une ligne qu'aucun certificat ne justifie n'est plus une autorité, la garder n'a pas de sens.*
2. **Justification multiple** (K tient deux mandats sur N, issus de deux émetteurs différents). La règle ci-dessus est conservatrice : il faut être ancêtre des **deux**. C'est le bon défaut — retirer une ligne qui reste justifiée par un mandat hors de son autorité, c'est précisément la révocation par omission.
3. **Rotation sans retrait** (rotation d'hygiène, aucun `dropped`). Aucune autorité de révocation n'est requise ; il suffit de détenir la DK. L'entrée `rotate` reste obligatoire (I5), avec `dropped: []`.

---

## 5. Recommandation

**Option A**, sous réserve d'une revue de la règle §4, de la forme exacte de
l'entrée gamma et de la compatibilité des vérifieurs.

L'argument principal est structurel : l'option A peut ajouter la preuve
d'autorité sans modifier les octets de header existants. Cette propriété doit
être confirmée par les vecteurs byte-exact concernés ; elle n'est pas déduite
d'un pourcentage global de conformité.

Le second argument est de cohérence : le kind `rotate` est déjà déclaré normativement par §7.3 et déjà présent dans l'enum du code. On n'ajoute pas un mécanisme, on branche celui que la spec avait prévu et qu'on n'a jamais câblé.

**Si le coût du balayage du log s'avère mesurable**, l'option C reste ajoutable ensuite de façon purement additive, sans invalider ce qui aura été construit sous A.

---

## 6. Ce que la proposition vise à débloquer, et son coût

**Débloque :**

- la rotation déléguée (`rotate_folder` pourrait accepter une chaîne au lieu de
  `&OwnerKeys`) — l'un des blocages à examiner pour *Core issuer* ;
- la règle de §5.5, aujourd'hui normative et inapplicable ;
- l'invariant I5 sur les rotations, aujourd'hui violé en silence ;
- les scénarios de révocation déléguée de l'app navigateur, qui buteraient dessus dès le premier parcours.

**Coût estimé :**

| Lot | Contenu |
|---|---|
| Spec | redline §3.4 (renvoi vers l'entrée `rotate`), §5.5 (règle de justification), §7.3 (forme de la charge utile `rotate`) |
| Core | `check_rotation` étendu ; nouvelle fonction de résolution `kid → mandats justifiants` ; contrôle d'ancêtre |
| Bundle | émission de l'entrée `rotate` dans `revoke.rs` et partout où un header change de version ; `rotate_folder` accepte une chaîne déléguée |
| Vecteurs | inventorier d'abord les fixtures dont le gamma ou le bundle byte-exact change ; ajouter ensuite un vecteur `g2b-rotation-authority.json` avec au minimum : retrait autorisé par l'émetteur, retrait refusé par un pair, retrait autorisé par le propriétaire, ligne orpheline, justification multiple, rotation d'hygiène sans retrait |
| BDD | scénarios correspondants dans `features/g-revocation.feature` — **écrits sans proxy**, chacun jouant son propre parcours |

Le coût de compatibilité reste donc une condition de décision, pas une
hypothèse déjà acquise.
