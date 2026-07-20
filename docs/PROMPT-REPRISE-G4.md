Tu prends la suite de la piste G : lot G4 (LA CÉRÉMONIE — le sous-mandat de
session frappé en wasm, lié au token OAuth) — seul, en loopback, zéro AWS,
dans UNE session (crates aithos-gateway + aithos-wasm ; jamais deux sessions
parallèles dessus ; la session piste P active sur aithos-provider ne compte
pas — ne touche JAMAIS à ses fichiers : rust/Cargo.toml, rust/Cargo.lock,
crates/aithos-provider/, vectors/README.md s'ils sont sales). G2, G6, G3 sont
clos (gates réels passés : Inspector + Claude Code + client OAuth générique).

⚠ PÉRIMÈTRE NEUF : G4 étend **aithos-wasm** (vs G1–G3 gateway-only). Lis sa
surface actuelle AVANT d'écrire. build_sub / verify_chain / verify_op
existent déjà dans aithos-core (spec §05) : la cérémonie les EXERCE, elle ne
les réécrit pas. Toute retouche de aithos-core (normalement AUCUNE) = rituel
vectors-first + BDD.

Lis dans cet ordre, INTÉGRALEMENT, avant d'écrire quoi que ce soit :
1. docs/HANDOFF-GATEWAY-G3-DONE-2026-07-17.md — TON précédent immédiat : ce
   que G3 a livré, et surtout LA COUTURE que G4 consomme —
   `Runner::agent_authority_ceiling` (la liaison token→chaîne INJECTABLE, à
   remplacer par le not_after du sous-mandat de session) et la page de
   consentement DEV de G3 (que la cérémonie remplace).
2. docs/HANDOFF-GATEWAY-HUB.md — l'état express 13ᵉ en tête + le plan G1–G9,
   LE document faisant foi : G4 y est spécifié (page servie par la gateway,
   wasm vérif+signature+frappe du sous-mandat vers gateway_pub TTL courte
   scopes ⊆ mandat issue non re-délégué, POST du sous-mandat à l'AS qui lie
   token ↔ session, flow CLI équivalent). Le gate G4 y est gravé.
3. docs/GAPS-DEMO-E2E.md §4.1 (les DEUX flux de livraison du mandat, décision
   déjà prise à assumer : **pack d'invitation** — mandat+keypair côté owner,
   envoyés par mail, DÉMO, marqué DEV, custody voyagée assumée — vs
   **pubkey-first** — la clé naît dans la page, la pubkey remonte, l'owner
   frappe, non-répudiation pleine) + beats 4 et 5c ; docs/INFRA-PROVIDER.md §5
   (OAuth = projection du mandat ; session = la personne frappe un sous-mandat
   de session vers gateway_pub ; actes sous owner → personne → session ;
   l'AS jamais chez Aithos) ; docs/STANDARDS-COMPAT.md C1 §6 (l'AS lie
   token↔session ; le token n'est qu'un pointeur de session périssable).
4. spec/04-mandates.md §4.7 (`session_bind: <pubkey>`, `max_sessions: N` — les
   clés de session) et §4.5 (l'algo verifier offline) ; spec/05-delegation.md
   INTÉGRALEMENT (§5.1 le droit `issue#depth`, §5.2 frappe d'un sous-mandat
   offline, §5.3 les invariants d'atténuation lien-à-lien — LE sous-mandat de
   session EN EST UN : scopes ⊆, fenêtre ⊆, contraintes ≥ strictes, issue non
   re-délégué, §5.4 la double barrière physique).
5. Le code aithos-wasm INTÉGRALEMENT (crates/aithos-wasm/ — sa surface
   actuelle : ce qu'il expose au JS, ce qu'il vérifie, comment il signe ; G4
   l'étend pour : vérif locale du mandat importé + signature du challenge +
   frappe du sous-mandat de session). Puis le code gateway dans cet ordre :
   src/oauth.rs (l'AS G3 À LA LETTRE — la couture `agent_authority_ceiling`
   côté caller à faire évoluer vers un résolveur token→chaîne-de-session, la
   page consentement DEV à remplacer par la cérémonie, le binding INJECTABLE
   posé exprès ; `exchange_code`/`refresh`/`mint_pair`/`validate_bearer`),
   src/core_bridge.rs (`agent_authority_ceiling`, `Mandate::build_sub`,
   verify_chain via le bridge, l'autorité injectable G6), src/config.rs (la
   stanza `as:` — issuer, TTLs, allowlist), src/proxy_mcp.rs (`router_oauth`,
   le gate bearer sur /mcp, `record_oauth_issue`), src/main.rs (le wiring du
   `run` : clé d'adapter née au 1er run, AS mergé sur le même listener),
   src/keyholder.rs + src/credentials.rs (LECTURE SEULE — intouchables),
   tests/features/gateway-oauth.feature (le contrat G3 — précédents H0/M1/
   G2-G6-G3), les steps oauth de tests/cucumber.rs (le pattern wire éphémère
   axum+reqwest, serve_with_as, l'horloge mutable), tests/e2e_hub.rs (harnais
   binaire réel).

Ordre d'attaque : contrat Gherkin d'abord, committé SEUL
(gateway-ceremony.feature : import du pack / pubkey-first, vérif wasm du
mandat importé, signature du challenge, frappe du sous-mandat de session
— TTL courte, scopes ⊆ mandat, `issue` non re-délégué, `session_bind` vers la
clé de session —, POST du sous-mandat à l'AS, liaison token ↔ chaîne de
session, ET TOUS les rejets : mandat importé invalide/expiré, sous-mandat qui
ÉLARGIT le parent — scope, fenêtre, contrainte, issue —, TTL de session
dépassé, révocation du sous-mandat qui COUPE le token au prochain acte, clé
de session absente). Les points NON TRANCHÉS se décident au contrat via
AskUserQuestion, JAMAIS en silence : (1) quel mode de livraison implémenter en
premier — pack DEV (aligné démo Léa) ou pubkey-first ; (2) le TTL par défaut
du sous-mandat de session (minutes ? heures ?) et son rapport au refresh
OAuth ≤ not_after ; (3) la route de la cérémonie (nouvelle page servie, ex.
/ceremony ou remplacement de /authorize) et comment elle remplace le
consentement DEV de G3 ; (4) comment le sous-mandat frappé atteint l'AS et
comment `validate_bearer` résout désormais token→chaîne-de-session (le token
porte-t-il l'id du sous-mandat ? un jti lié ?) ; (5) ce que aithos-wasm doit
exposer EXACTEMENT au JS (verify_mandate, sign_challenge, build_session_sub)
et où vit la clé de session (éphémère, dans la page) ; (6) le flow CLI
équivalent pour les devs ; (7) `max_sessions` par défaut. Puis impl par
tranche, détag progressif, suite complète verte à CHAQUE détag, gate réel
(depuis un navigateur vierge : pack → session active en < 2 min ; le
sous-mandat apparaît dans les certs ; sa révocation coupe la session) contre
le vrai binaire dans le conteneur cloud. STOP à chaque gate pour validation
Mathieu. Mode absent si le pont flappe : defaults @wip committés, zéro impl
des points non tranchés ; reposer UNE fois à la reconnexion, pas de boucle.

Interdits absolus : keyholder.rs et credentials.rs ne bougent pas d'un octet ;
la clé de signature des tokens = clé d'adapter (secret gateway ordinaire, née
0600 au 1er run, JAMAIS un objet protocole) ; la clé de session est éphémère
et ne remplace jamais la clé d'adapter ; un token ne remplace JAMAIS la
vérification de chaîne — le sous-mandat de session est revérifié à chaque
acte (owner → personne → session) ; le sous-mandat de session ne peut
qu'ATTÉNUER le mandat parent (§5.3 : scopes ⊆, fenêtre ⊆, contraintes ≥
strictes, issue non re-délégué) — un sous-mandat qui élargit est REFUSÉ en
nommant la famille élargie ; as: absent = comportement actuel byte-identique ;
préfixes réservés inchangés ; jamais de réécriture d'appel ; refus
pédagogiques ; fail-closed partout ; aucun secret, token, clé de session ou
code dans logs/erreurs/panics ; entropie injectée uniquement (EntropySource) ;
toute retouche core (normalement AUCUNE) = rituel vectors-first + BDD ; pas de
merge main sans gate humain ; cucumber gateway SÉQUENTIEL ; le chemin chaud de
la démo Léa ne bouge pas (8 beats + e2e_demo_lea verts à chaque commit).

Baseline à préserver à chaque commit (revalider À L'IDENTIQUE avant tout
travail) : gateway 82 unit / 4 CLI / 152 scénarios-790 steps / 6 e2e (dont
e2e_demo_lea) / 7 owner / 5 équivalence ; core+bundle+cli 100 tests +
229/906 cucumber ; clippy -D warnings + fmt clean (core, bundle, gateway ;
+ aithos-wasm si tu le touches). Branche feat/obligations, HEAD d'entrée
9610fe1 (+ le commit docs de clôture G3 au-dessus si présent) — si des commits
P ou docs se sont posés au-dessus, vérifie qu'ils sont disjoints du
gateway/wasm et continue.

Protocole d'environnement (13ᵉ session : hybride confirmé) : sondes egress +
unlink SUR LE MONTAGE d'abord (pas /tmp). Si egress 000 + unlink DENIED + pas
de toolchain VM → protocole cloud+janitor GATEWAY-HANDOFF §5 À LA LETTRE : tar
du working tree depuis le montage (exclut rust/target*, .git, _*, ui-mockup,
.DS_Store, cargo-linux) → device_stage_files → build/test cloud (rustc 1.95.0,
CARGO_INCREMENTAL=0, suite gateway ~2 min à froid ; ATTENTION le tar embarque
l'état P sale — il compile, ne pas s'en étonner, ne pas y toucher) → retours
device_commit_files fichier par fichier + sha256 croisé dans les deux sens à
chaque transfert → staging git sélectif, fichiers nommés un à un (c'est ce qui
protège P) → mv .git/*.lock _gitjunk/ avant CHAQUE commande git écrivante,
jamais de git status (lectures : git --no-optional-locks). Le conteneur cloud
a le réseau, npx (MCP Inspector) ET le CLI claude : les gates contre clients
réels se font LÀ (flow PKCE/cérémonie scripté + Inspector). Le flow navigateur
COMPLET côté Claude custom connector réel (callback claude.ai) attend le
tunnel G1 — le dire au gate plutôt que le simuler ; les scripts du test
navigateur local sont dans _transfer/g3-browser/. Scories assumées (ignorer) :
_transfer/, _gitjunk/, _to_delete/.

En fin de session : suites complètes + clippy + fmt, synchro sha-croisée,
état express en tête de docs/HANDOFF-GATEWAY-HUB.md, un bloc §6 dans
GATEWAY-HANDOFF.md tracké, et un handoff de reprise (untracked, HEAD d'entrée
exact). G4 conditionne G5 (multi-principal : une chaîne de session PAR token,
le ceiling et le binding token→chaîne se spécialisent par session via la même
couture injectable). G7 et G8.a/c/d restent parallélisables en sessions
dédiées ; le lot core « résolution self déléguée » débloque le self-serves
@wip.
