# Installer `aithos` et créer un Ethos avec HashiCorp Vault

Ce parcours utilise de vraies clés aléatoires générées par l'OS. Aucun seed
n'est affiché, passé dans les arguments ou écrit dans le bundle. HashiCorp
Vault KV v2 conserve le master seed et la clé de succession ; le profil local
ne contient que leur référence non secrète.

> Le mode dev de Vault ci-dessous est un vrai serveur Vault et le vrai wire KV
> v2, mais pas une configuration de production : token root connu, stockage en
> mémoire et HTTP loopback. Il sert à valider l'intégration immédiatement.

## 1. Installer la CLI depuis le checkout

Depuis `code/aithos-core/rust/` :

```bash
cargo install --locked --force --path crates/aithos-cli
aithos --version
```

Cargo installe le binaire dans `~/.cargo/bin/aithos`. Ce répertoire doit être
présent dans `PATH`.

## 2. Lancer un vrai Vault local

```bash
docker run --rm --name aithos-vault-dev --cap-add=IPC_LOCK \
  -e VAULT_DEV_ROOT_TOKEN_ID=aithos-dev-root \
  -p 127.0.0.1:8200:8200 hashicorp/vault
```

Dans un second terminal :

```bash
export VAULT_ADDR=http://127.0.0.1:8200
export VAULT_TOKEN=aithos-dev-root
```

## 3. Créer l'Ethos

```bash
aithos init --key-store vault
```

Par défaut :

- profil : `default` ;
- bundle macOS : `~/Library/Application Support/Aithos/ethos/default/bundle` ;
- secret Vault : mount `secret`, path `aithos/ethos/default` ;
- token : lu depuis `VAULT_TOKEN`, jamais persisté par Aithos.

Personnalisation :

```bash
aithos --profile entreprise init --key-store vault \
  --vault-address https://vault.example.com \
  --vault-mount secret \
  --vault-path teams/ai/aithos/entreprise \
  --vault-token-env VAULT_TOKEN
```

## 4. Vérifier la custody et le bundle

```bash
aithos status
```

Résultat attendu :

```text
key_store: vault-kv2
custody: OK
edition_chain: OK
gamma_chain: OK
```

Vérification côté Vault sans afficher les seeds :

```bash
docker exec -e VAULT_ADDR=http://127.0.0.1:8200 \
  -e VAULT_TOKEN=aithos-dev-root aithos-vault-dev \
  vault kv metadata get -mount=secret aithos/ethos/default
```

## 5. Utiliser l'Ethos sans seed ni chemin de bundle

```bash
aithos section-add public profil/bio \
  --title "Présentation" --body "Mon Ethos réel."

aithos section-add circle projets/note \
  --title "Note" --tags demo --body "Contenu privé."

aithos zone-show circle
aithos edition-publish
aithos edition-verify
aithos log-show
aithos log-verify
```

Chaque commande charge le profil, demande le secret au backend de custody,
reconstruit les clés en mémoire et les abandonne à la fin du processus.

## 6. Tester le fail-closed

Arrêter Vault :

```bash
docker stop aithos-vault-dev
```

Puis :

```bash
aithos status
```

La commande doit échouer. Une indisponibilité de Vault ne bascule jamais sur
une clé locale et n'autorise aucune opération owner.

## Notes de production

- Utiliser HTTPS et une PKI vérifiée.
- Remplacer le token root par une policy limitée au seul chemin de l'Ethos.
- Utiliser un token court renouvelable, AppRole, Kubernetes auth ou l'identité
  de workload de l'entreprise.
- Isoler idéalement la clé de succession sous un chemin et une policy plus
  stricts que le master seed quotidien.
- Activer l'audit device Vault, la réplication et les procédures de recovery.
