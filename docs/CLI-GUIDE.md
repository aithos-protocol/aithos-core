# Guide de test manuel — CLI aithos-core

> Tout est copier-coller depuis `code/aithos-core/`, sur un bundle jetable dans
> `/tmp/aithos-demo`. Chaque bloc indique ce que tu dois voir. Les seeds sont
> les seeds DEV des vecteurs (déterministes) — jamais ça en prod, évidemment.

## 0. Préparation

```bash
cargo build --manifest-path rust/Cargo.toml -p aithos-cli
alias ac="$(pwd)/rust/target/debug/aithos-core"

# Les acteurs (seeds DEV) :
export S=000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f  # owner
export A=$(printf 'a1%.0s' $(seq 32))   # agent
export H=$(printf 'b2%.0s' $(seq 32))   # helper (délégué)
export D=/tmp/aithos-demo
rm -rf $D
```

`ac --help` liste tous les verbes ; `ac <verbe> --help` détaille chacun.

## 1. Identité & bundle

```bash
ac init --seed-hex $S --succession-seed-hex $(printf '09%.0s' $(seq 32)) --dir $D
ac edition-verify --dir $D        # → edition chain: OK
ac log-show --dir $D              # → log vide, head: (rien)
```

Regarde le disque : `find $D -type f` — did.json, headers `e/*/header.json`
(dont `e/x/` = vault d'audit), index, manifeste. Ouvre `$D/e/self/index.json` :
aucun nom ne fuit, que des sids opaques.

## 2. Contenu — les trois zones

```bash
ac section-add --dir $D --seed-hex $S public  bio           --title bio    --body "Bio publique."
ac section-add --dir $D --seed-hex $S circle  projets/note1 --title note --tags toto --body "corps secret"
ac section-add --dir $D --seed-hex $S self    journal/jour1 --title intime --body "jamais signé"
ac zone-show   --dir $D --seed-hex $S circle
ac section-read --dir $D circle projets/note1 --seed-hex $S     # → corps secret
ac section-read --dir $D public bio                             # → sans clé !
ac log-show --dir $D
```

`log-show` : 3 entrées `section.add`. La publique montre son target clair
(`/e/public/...`), circle et self affichent `(sealed)` — **le log ne révèle
jamais quoi**. Vérifie sur disque : `cat $D/gamma/*.jsonl` — du JCS chaîné,
`body_enc` chiffré, signatures owner `#content`.

## 3. Éditions — la chaîne inviolable

```bash
ac edition-publish --dir $D --seed-hex $S
ac edition-verify  --dir $D                    # → OK
ac log-verify      --dir $D                    # → gamma chain: OK (sans clé !)

# Sabotage : altère un octet d'un segment gamma, la vérif casse
sed -i '' 's/"seq"/"sEq"/' $D/gamma/*.jsonl 2>/dev/null || true
python3 -c "
import glob
f=glob.glob('$D/gamma/*.jsonl')[0]; b=bytearray(open(f,'rb').read())
i=b.find(b'\"value\":\"')+9; b[i] = ord('1') if b[i]==ord('0') else ord('0')
open(f,'wb').write(bytes(b))"
ac log-verify --dir $D                         # → Error (signature/chaîne)
git checkout -- . 2>/dev/null; rm -rf $D; # repars de zéro : rejoue les blocs 0→3 sans le sabotage
```

## 4. Mandat de lecture + lecture agent

```bash
ac grant --dir $D --seed-hex $S --agent-seed-hex $A projets --tag toto --ttl-days 7
export CERT=$(ls -t $D/certs/*.json | head -1)
ac mandate-verify --dir $D --cert $CERT --at $(date -u +%Y-%m-%dT%H:%M:%SZ)   # → OK
ac section-read-agent --dir $D --cert $CERT --agent-seed-hex $A \
    --at $(date -u +%Y-%m-%dT%H:%M:%SZ) projets/note1          # → corps secret
ac mandate-verify --dir $D --cert $CERT --at 2027-01-01T00:00:00Z  # → Error (expiré)
ac log-show --dir $D    # le grant lui-même est loggé (kind grant)
```

## 5. Le compteur agentique (F)

```bash
ac grant-act --dir $D --seed-hex $S --agent-seed-hex $A --max-actions 3 gmail reply
export CACT=$(ls -t $D/certs/*.json | head -1)
ac action --dir $D --cert $CACT --agent-seed-hex $A gmail reply --args "mail 1"
ac action --dir $D --cert $CACT --agent-seed-hex $A gmail reply --args "mail 2"
ac action --dir $D --cert $CACT --agent-seed-hex $A gmail reply --args "mail 3"
ac action --dir $D --cert $CACT --agent-seed-hex $A gmail reply --args "mail 4"
# → Error: GammaBudgetExhausted("...max_actions 3 spent")
```

Le refus ne vient pas d'un serveur : `ac log-verify` puis compte toi-même les
entrées `action` dans `log-show`. **Le log EST le compteur.**

## 6. Dead-man switch (heartbeat, en accéléré)

```bash
ac grant-act --dir $D --seed-hex $S --agent-seed-hex $H --label head \
    --heartbeat-every 10s --heartbeat-grace 5s --ttl-days 30 gmail '*'
export CHB=$(ls -t $D/certs/*.json | head -1)
ac heartbeat --dir $D --seed-hex $S --seq 1
ac action --dir $D --cert $CHB --agent-seed-hex $H gmail send --args "vivant"   # → OK
sleep 16
ac action --dir $D --cert $CHB --agent-seed-hex $H gmail send --args "trop tard"
# → Error: GammaHeartbeatStale — l'owner s'est tu, l'agent est suspendu
ac heartbeat --dir $D --seed-hex $S --seq 2
ac action --dir $D --cert $CHB --agent-seed-hex $H gmail send --args "reprise"  # → OK
```

## 7. Budgets par profil (F+)

```bash
ac grant-act --dir $D --seed-hex $S --agent-seed-hex $A --label llm \
    --budgets-json '[{"id":"gemma","models":["gemma"],"token_budget":25000}]' gmail '*'
export CBUD=$(ls -t $D/certs/*.json | head -1)
ac inference --dir $D --cert $CBUD --agent-seed-hex $A --tokens-in 11000 --tokens-out 1000 --budget-ref gemma prov gemma
ac inference --dir $D --cert $CBUD --agent-seed-hex $A --tokens-in  8000 --tokens-out 1000 --budget-ref gemma prov gemma
ac inference --dir $D --cert $CBUD --agent-seed-hex $A --tokens-in  4900 --tokens-out  100 --budget-ref gemma prov gemma
# → Error: "profile 'gemma' token budget 25000 spent (21000 used)"

ac action --dir $D --cert $CBUD --agent-seed-hex $A --budget-ref gemma --model gpt-oss --tokens 10 gmail reply
# → Error: "model 'gpt-oss' not allowed by 'gemma'"
ac action --dir $D --cert $CBUD --agent-seed-hex $A gmail reply --args x
# → Error: "budgets present but no budget_ref cited"
```

## 8. Fenêtres absolues (F+)

```bash
# Une fenêtre de 2 minutes qui s'ouvre MAINTENANT :
export NOW=$(date -u +%Y-%m-%dT%H:%M:%SZ)
ac grant-act --dir $D --seed-hex $S --agent-seed-hex $A --label fen \
    --windows-json "[{\"anchor\":\"$NOW\",\"duration\":\"2m\"}]" gmail reply
export CWIN=$(ls -t $D/certs/*.json | head -1)
ac action --dir $D --cert $CWIN --agent-seed-hex $A gmail reply --args "dans la fenêtre"   # → OK
sleep 125
ac action --dir $D --cert $CWIN --agent-seed-hex $A gmail reply --args "trop tard"
# → Error: "outside every active window"
```

(Fenêtre périodique : ajoute `"period":"7d"` et éventuellement `"until"` ou
`"count"` — arithmétique pure, aucun fuseau nulle part.)

## 9. Args scellés + audit a posteriori (F+)

```bash
ac grant-act --dir $D --seed-hex $S --agent-seed-hex $A --label audite --audit gmail reply
export CAUD=$(ls -t $D/certs/*.json | head -1)
ac action --dir $D --cert $CAUD --agent-seed-hex $A \
    --args-json '{"recipient":"alice@example.com","subject":"re: devis"}' gmail reply
ac log-show --dir $D | tail -2
# l'entrée porte un args_hash clair ; les args eux-mêmes sont scellés
grep -o 'alice' $D/gamma/*.jsonl || echo "alice n'apparaît nulle part sur disque ✔"
ac log-audit --dir $D --seed-hex $S
# → l'owner rouvre les args, revérifie le hash : "all consistent"
ac log-audit --dir $D --seed-hex $S --cert $CAUD   # + prédicats action_params du cert
```

## 10. Recherche dans le log (F/F+)

```bash
ac log-query --dir $D --seed-hex $S --kind action                 # toutes les actions
ac log-query --dir $D --seed-hex $S --kind ethos.write            # CLASSE = tous les section.*
ac log-query --dir $D --seed-hex $S --folder projets              # par sous-arbre (corps déchiffrés)
ac log-query --dir $D --seed-hex $S --since $(date -u -v-1H +%Y-%m-%dT%H:%M:%SZ)  # dernière heure
ac log-query --dir $D --seed-hex $S --mandate $(basename $CACT .json | sed 's/^/EDIT_ME_/')  # par mandat : mets l'id du cert
ac edition-publish --dir $D --seed-hex $S && ac edition-verify --dir $D
```

Note `--folder` : la query retrouve les mutations **scellées** de ce sous-arbre
via les hints — seul un détenteur de clés (ici l'owner) peut le faire.

## 11. Move = rotation (G, spec 02.9)

```bash
ac section-add --dir $D --seed-hex $S circle archives/old/note1 --title n --body "secret"
ac section-add --dir $D --seed-hex $S circle projets/keep --title n --body "ancre"
ac grant --dir $D --seed-hex $S --agent-seed-hex $A1 archives/old --ttl-days 7   # ligne directe
ac grant --dir $D --seed-hex $S --agent-seed-hex $A2 archives --ttl-days 7      # ancien parent
ac move --dir $D --seed-hex $S archives/old --under projets
# → "moved archives/old under projets — rotated, wrap posted, subtree re-encrypted"
ac section-read-agent --dir $D --cert $C1 --agent-seed-hex $A1 --at $T projets/old/note1
# → le corps : la ligne directe survit, même keypair, nouvelle adresse
ac section-read-agent --dir $D --cert $C2 --agent-seed-hex $A2 --at $T projets/old/note1
# → Error "not covered" : le grant sur l'ancien parent ne suit pas le nœud
```

Le déplacer VERS un dossier partagé le partage (wrap sous le nouveau parent) ;
le déplacer HORS d'un dossier partagé le dé-partage (rotation). Comme un objet
physique qui change de tiroir. Couper quelqu'un explicitement = `revoke`.

## 12. Obligations — approbation humaine & co_sign (G+, spec 04.12)

```bash
export AP=$(printf 'b5%.0s' $(seq 32))   # la clé DEVICE de l'approbateur
PUB=$(ac approve --approver-seed-hex $AP --mandate - --key-only publish)
echo $PUB                                # → z6Mk… : à épingler au grant

ac grant-act --dir $D --seed-hex $S --agent-seed-hex $A \
  --obligations-json "[{\"id\":\"publish-approval\",\"check\":\"human.approve\",\
\"attestor\":[\"$PUB\"],\"applies_to\":\"act.x.social.publish\",\
\"verdict\":\"approve\",\"max_age\":\"5m\"}]" social '*'
export CACT=$D/certs/<le cert affiché>.json ; export MID=$(basename $CACT .json)

ac action --dir $D --cert $CACT --agent-seed-hex $A social publish --args "Ship it"
# → Error GammaObligationUnsatisfied("…no receipt…") : fail-closed, rien d'appendé

ac approve --approver-seed-hex $AP --obligation publish-approval --mandate $MID \
  --args "Ship it" --presented "PUBLISH: Ship it" publish > /tmp/receipt.json
# → le reçu signé {obligation, args_hash, verdict, presented_digest, at, sig}
#   (presented_digest = WYSIWYS : le hash de CE que le device a montré est
#    DANS les octets signés ; jamais de seed en sortie)

ac action --dir $D --cert $CACT --agent-seed-hex $A social publish \
  --args "Ship it" --check-json "$(cat /tmp/receipt.json)"
# → action logged — le reçu ride dans checks[] de l'entrée

ac action --dir $D --cert $CACT --agent-seed-hex $A social publish \
  --args "Something else" --check-json "$(cat /tmp/receipt.json)"
# → Error "args_hash does not match" : un reçu est single-use, lié aux args

ac approve --approver-seed-hex $AP --obligation publish-approval --mandate $MID \
  --args "Old news" --at $(date -u -v-12M +%Y-%m-%dT%H:%M:%SZ) publish > /tmp/stale.json
ac action --dir $D --cert $CACT --agent-seed-hex $A social publish \
  --args "Old news" --check-json "$(cat /tmp/stale.json)"
# → Error "receipt is stale (720s > 300s)" : l'approbation ne vieillit pas

# counter_sign = la même primitive, instance owner (clé content, id co_sign) :
ac grant-act --dir $D --seed-hex $S --agent-seed-hex $A --counter-sign send gmail '*'
export CCS=$D/certs/<cert>.json ; export MCS=$(basename $CCS .json)
ac action --dir $D --cert $CCS --agent-seed-hex $A gmail send --args "wire"   # → refusé
ac action --dir $D --cert $CCS --agent-seed-hex $A gmail reply --args "re"    # → passe (non binding)
ac approve --owner-seed-hex $S --mandate $MCS --args "wire" --presented "SEND: wire" send > /tmp/co.json
ac action --dir $D --cert $CCS --agent-seed-hex $A gmail send --args "wire" \
  --check-json "$(cat /tmp/co.json)"                                          # → logged
```

Le protocole vérifie une signature liée (mandat feuille + action + args + at),
jamais la logique : guardrail, humain qui tape « approve » ou second agent,
même wire. La logique vit dans l'attestor, hors protocole.

## 13. Racines Merkle — preuves & diff (H1, spec 02.10)

```bash
ac section-add --dir $D --seed-hex $S circle projets/note1 --title N1 --body "secret"
ac section-add --dir $D --seed-hex $S circle projets/note2 --title N2 --body "autre"
ac edition-publish --dir $D --seed-hex $S
python3 -c "import json; print(json.load(open('$D/manifest.json'))['roots'])"
# → 4 racines (public, circle, self, vault) À CÔTÉ des pins plats ;
#   self vide = 32×00

ac prove --dir $D circle projets/note1
# → le proof JSON (payload = row+header_hash, steps node/wrap) ; la CLI le
#   re-vérifie hors-ligne contre la racine signée avant de l'imprimer

ac section-add --dir $D --seed-hex $S circle projets/note3 --title N3 --body "3e"
ac edition-publish --dir $D --seed-hex $S
ac edition-diff --dir $D
# → descente de racines : circle:z changed, le dossier changed, la
#   nouvelle section added — rien d'autre, aucune autre zone

python3 - <<'EOF'   # trafique une row d'index…
import json; p='$D/e/circle/index.json'; v=json.load(open(p))
v['sections'][0]['title']='HACKED'; json.dump(v, open(p,'w'))
EOF
ac edition-verify --dir $D
# → Error "pinned file altered" (les pins plats) ; la racine recalculée
#   divergerait aussi (MerkleRootMismatch) — ceinture et bretelles
```

Une preuve montre l'inclusion dans une édition signée, jamais la fraîcheur
(limite honnête §02.10) : la staleness reste bornée par `freshness` et les
chaînes édition + gamma.

## Récap des invariants que tu viens de toucher

| Bloc | Invariant prouvé |
|---|---|
| 2-3 | le log révèle l'acte, jamais le contenu ; chaîne write-once |
| 4 | mandat = certificat pur, vérifiable hors-ligne, expire tout seul |
| 5 | I5 : pas d'entrée → pas d'action ; le budget est un tally de fichiers |
| 6 | l'autonomie est bornée à la présence de l'owner |
| 7 | budgets par profil en OU, modèle allow-listé, tokens comptés du log seul |
| 8 | le temps du verifier est arithmétique — zéro fuseau, zéro DST |
| 9 | l'audit rouvre ce que l'étranger ne voit pas ; hash = intégrité |
| 10 | la recherche suit ce que tes clés ouvrent, rien de plus |
| 11 | le périmètre suit le nœud, pas l'adresse ; la dérivation ne se désapprend pas |
| 12 | un permis peut exiger sa décharge : reçu signé, lié, single-use, vérifié des fichiers seuls |
| 13 | tout mirror peut prouver une row en O(log n) sans être cru ; un splice meurt sur le domaine |
