# CLAUDE.md — Convertisseur RSS en Rust

## 1. Contexte

Ce projet fait partie d'une veille technologique basée sur des flux RSS et un agrégateur.
Certaines sources n'ont pas de flux RSS natif. Ce programme les convertit : il lit une page web,
repère la liste d'articles avec des sélecteurs CSS, et écrit un fichier RSS 2.0 par source.

Il remplace un service en ligne (FetchRSS) limité à 5 flux gratuits.

## 2. Ton rôle : coder ET enseigner

L'utilisateur **apprend Rust avec ce projet**. Il n'a jamais codé en Rust.
Il est développeur web (frontend / full-stack) : il connaît le HTML, les sélecteurs CSS, le JavaScript et le PHP.

Règles de travail :

- **Avance par petites étapes.** Une étape = un objectif clair, testable, puis une pause.
- **Explique chaque notion Rust nouvelle** la première fois qu'elle apparaît
  (ownership, emprunt `&`, `Result` et `?`, `Option`, `struct`, `impl`, traits, `Vec`, itérateurs, `match`, crates, `Cargo.toml`…).
  Fais le lien avec ce qu'il connaît déjà (JS, PHP) quand c'est utile.
- **Explique les erreurs du compilateur** : ce qu'elles veulent dire, pourquoi Rust refuse, comment corriger.
- **Ne génère pas tout d'un coup.** Écris le code d'une étape, explique-le, lance-le, puis attends son accord avant l'étape suivante.
- **Préfère le code simple et lisible** au code idiomatique avancé. Pas d'`async` en v1 (voir §6).
- À la fin de chaque étape, fais un **résumé court** : ce qui a été fait, ce qu'il a appris, la prochaine étape.

## 3. Langue

- **Explications, résumés, questions : en français.** Phrases courtes et claires.
- **Code, noms de variables, commentaires dans le code, messages de commit : en anglais.**

## 4. Environnement

- Système : **Windows**.
- Éditeur : **Neovim (NvChad)**. Claude Code est utilisé dans le terminal.
- Rust **est installé** (phase 0 faite) : toolchain `stable-x86_64-pc-windows-msvc` + « Build Tools » C++ de Microsoft.
  Ne pas repasser en toolchain `gnu`. La variable d'environnement `CC=gcc` (TDM-GCC, utilisée ailleurs) fait échouer la compilation des crates qui contiennent du C :
  c'est pour ça que `reqwest` utilise `native-tls` au lieu de `rustls`/`aws-lc`.
- Git : compte GitHub existant, **peu d'expérience avec Git** (voir §9).

Les commandes données doivent fonctionner dans PowerShell.
**Ne lance jamais un installateur ou une commande qui modifie le système sans l'accord explicite de l'utilisateur.**
Explique d'abord ce que fait la commande.

## 5. Périmètre de la v1

### Dans la v1

Les 6 sources ci-dessous, **uniquement celles dont le contenu est présent dans le HTML brut** (sans JavaScript).

| Nom (id) | Page à convertir | Rubrique | Priorité |
|---|---|---|---|
| `tokens-studio` | https://tokens.studio/blog | Figma | 1 — source de départ |
| `interaction-design-foundation` | https://www.interaction-design.org/literature | UX | 2 |
| `dribbble-stories` | https://dribbble.com/stories | UI | 2 |
| `figma-forum` | https://forum.figma.com/product-updates-3 | Figma | 2 |
| `laws-of-ux` | https://lawsofux.com/ | UX | 3 — peu de nouveautés |
| `figma-config` | https://config.figma.com/ | Figma | 3 — événement annuel |

Les sélecteurs CSS de chaque source **ne sont pas encore connus**.
Pour chaque source : télécharge la page, inspecte le HTML, propose les sélecteurs et explique ton choix.

**Si le contenu d'une page n'est pas dans le HTML brut** (page construite en JavaScript) :
ne cherche pas de contournement. Signale-le, marque la source comme `needs_js` dans la config, et passe à la suivante.

### Hors v1 (ne pas faire sans demande)

- Behance (pas de page stable à convertir).
- Navigateur headless (`chromiumoxide`, `headless_chrome`…) pour les sites en JavaScript.
- Publication en ligne des fichiers XML et planification automatique (voir §11).
- Base de données, serveur HTTP, interface web.

## 6. Choix techniques

Crates à utiliser :

| Besoin | Crate |
|---|---|
| Télécharger une page | `reqwest` avec la feature `blocking` |
| Extraire avec des sélecteurs CSS | `scraper` |
| Écrire le flux RSS 2.0 | `rss` |
| Lire la config | `serde` (feature `derive`) + `toml` |
| Lire les dates | `chrono` |
| Gérer les erreurs simplement | `anyhow` |

- **Pas d'`async` / `tokio` en v1** : le mode `blocking` de `reqwest` suffit pour 6 pages et reste plus simple à apprendre.
  On pourra passer en asynchrone plus tard, comme exercice.
- N'ajoute pas d'autre crate sans expliquer pourquoi et demander l'accord.

## 7. Fonctionnement attendu

1. Lire `sources.toml`.
2. Pour chaque source active (pas `needs_js`, pas `enabled = false`) :
   1. télécharger la page ;
   2. appliquer les sélecteurs (article, titre, lien, date, résumé) ;
   3. transformer les liens relatifs en liens absolus ;
   4. construire un flux RSS 2.0 ;
   5. l'écrire dans `feeds/<id>.xml`.
3. Afficher un bilan dans le terminal : une ligne par source, avec le nombre d'articles trouvés.

Format de config attendu (à ajuster si besoin, en expliquant) :

```toml
[[source]]
id = "tokens-studio"
name = "Tokens Studio Blog"
url = "https://tokens.studio/blog"
enabled = true
needs_js = false

[source.selectors]
item = "article"
title = "h2"
link = "a"            # optional: element holding the href; if absent, the href of the item itself
date = "time"         # optional: datetime attribute, or the element text
summary = "p"         # optional
```

## 8. Règles de robustesse

- **Une source en erreur ne bloque pas les autres.** Affiche l'erreur et continue.
- **Zéro article trouvé = avertissement visible** (le site a probablement changé son HTML). Ne pas écraser l'ancien fichier XML dans ce cas.
- Le `guid` de chaque article = son URL absolue.
- Date absente ou illisible : ne pas inventer de date, laisser le champ vide.
- Requêtes polies : un `User-Agent` honnête qui nomme le projet, un timeout (ex. 20 s), **une seule requête par source et par exécution**.
- Pendant le développement, enregistre le HTML téléchargé dans `cache/` pour tester les sélecteurs sans re-télécharger la page à chaque essai.

## 9. Git

L'utilisateur débute avec Git. Pour chaque commande Git : dis ce qu'elle fait, en une phrase.

- Un commit à la fin de chaque étape qui fonctionne.
- Messages de commit courts, en anglais, style « Conventional Commits » (`feat:`, `fix:`, `docs:`, `chore:`).
- `.gitignore` : `target/`, `cache/` et `feeds/` (fichiers générés).
- **Ne fais jamais de `git push`, de création de dépôt distant, ni de commande destructive** (`reset --hard`, `push --force`…) sans accord explicite.

## 10. Qualité

Avant chaque commit :

- `cargo fmt`
- `cargo clippy` — explique chaque avertissement et sa correction.
- `cargo test` — ajoute au moins un test par fonction d'extraction, en utilisant un petit extrait HTML fixe (pas de requête réseau dans les tests).

## 11. Plan de travail

Chaque phase se termine par une démonstration qui marche, un résumé, et l'accord de l'utilisateur.

**Phase 0 — Installation sur Windows**
Installer Rust avec `rustup` (et les « Build Tools » C++ de Microsoft si nécessaire).
Vérifier avec `rustc --version` et `cargo --version`.
Installer `rust-analyzer` pour Neovim (via `rustup component add rust-analyzer` ou Mason) et vérifier qu'il fonctionne dans NvChad.

**Phase 1 — Projet vide**
`cargo new`, présentation de `Cargo.toml` et de `src/main.rs`, premier `cargo run`, `git init`, premier commit.

**Phase 2 — Une source de bout en bout**
`tokens-studio` uniquement, sélecteurs écrits directement dans le code.
Objectif : `feeds/tokens-studio.xml` valide, lisible dans un agrégateur.

**Phase 3 — Configuration**
Sortir l'URL et les sélecteurs dans `sources.toml`. Introduire `struct` + `serde`.

**Phase 4 — Les autres sources**
Ajouter les 5 autres une par une. Pour chacune : vérifier si le HTML brut contient les articles, proposer les sélecteurs, tester.

**Phase 5 — Robustesse et tests**
Règles du §8, tests du §10, bilan dans le terminal.

**Phase 6 — Hébergement sur VPS (ne pas commencer sans demande)**
Publication des fichiers XML et exécution **1 fois par jour** sur un VPS Infomaniak.
Choix, règles et plan détaillé (phases H0 à H6) : voir §13.

## 12. Décisions encore ouvertes

- Que faire des sources marquées `needs_js` (aucune pour l'instant ; `figma-config` est désactivée pour une autre raison : ce n'est pas une liste d'articles).
- Ajout éventuel de Behance dans une v2.
- Hébergement : décisions encore ouvertes au §13.6.
  (Décidé : publication sur un VPS Infomaniak, voir §13.)

## 13. Hébergement sur VPS (phase 6)

Cette section remplace les pistes de la phase 6 (§11). La phase 6 ne commence que sur demande de l'utilisateur.

### 13.1 Contexte et choix faits

- Un **VPS payant chez Infomaniak** : offre **VPS Cloud** de base, avec une IPv4 publique. La contrainte de gratuité est levée pour l'hébergement.
- **Pas de Docker pour conv_rss** : installation directe sur le système, planification sur le VPS.
  Docker sera installé plus tard, pour un **futur projet** sur le même VPS.
  Attention à ce moment-là : un port publié par un conteneur (`-p 8080:80`) peut contourner `ufw` et être accessible depuis Internet.
  Il faudra le prévoir (publier sur `127.0.0.1`, ou configurer Docker et `ufw` ensemble).
- **Pas de nom de domaine.** Adresse publique via **sslip.io** : `https://<ip-avec-des-tirets>.sslip.io/` (ex. `203-0-113-5.sslip.io`).
- Serveur web : **Caddy**, qui obtient et renouvelle seul le certificat HTTPS (Let's Encrypt).
- **Compilation sur le VPS** : `git clone` depuis GitHub, puis `cargo build --release`.
- Exécution **1 fois par jour**, planifiée sur le VPS.
- Système proposé : **Ubuntu LTS** (à confirmer en phase H0).

### 13.2 Ton rôle

Même méthode que pour Rust (§2) : petites étapes, une démonstration, un résumé, puis l'accord de l'utilisateur.

- L'utilisateur a **des notions de Linux** (`ls`, `cd`, `nano`, une connexion SSH), mais il n'a **jamais configuré de serveur**.
  Explique chaque commande Linux nouvelle et chaque option (`sudo`, `chmod`, `systemctl`…), en faisant le lien avec ce qu'il connaît.
- Explique **chaque fichier de configuration ligne par ligne** (`Caddyfile`, `sshd_config`, crontab ou timer systemd…).
- **Les commandes sur le VPS sont tapées par l'utilisateur** dans sa session SSH. Donne la commande, explique-la, puis attends le résultat.
  Ne te connecte jamais toi-même au VPS sans son accord explicite.
- **Le compte Infomaniak, le choix de l'offre et le paiement sont faits par l'utilisateur seul.** Ne saisis jamais de données de paiement.
- **Ne jamais afficher, copier dans le chat ou committer** une clé privée, un mot de passe ou un token.
- Avant chaque commande qui modifie le serveur, dis ce qu'elle change et comment revenir en arrière.

### 13.3 Règles de sécurité (non négociables)

- **SSH par clé uniquement** (clé `ed25519` générée sur Windows).
  Désactiver la connexion `root` et les mots de passe **seulement après avoir vérifié que la clé fonctionne**.
  Garder une session SSH ouverte pendant le test, pour ne pas s'enfermer dehors.
- **Pare-feu** (`ufw`) : seuls les ports 22 (SSH), 80 et 443 (web) sont ouverts.
- **Mises à jour de sécurité automatiques** (`unattended-upgrades`).
- Deux utilisateurs :
  - un utilisateur **admin** personnel, avec `sudo` ;
  - un utilisateur **`conv_rss`**, sans `sudo`, qui fait tourner le programme.
  Rien ne tourne en `root`, sauf Caddy, qui gère lui-même ses droits.
- Caddy ne sert **que** le dossier des flux. Aucun autre fichier du serveur n'est exposé.

### 13.4 Modifications du programme

- Ajouter le lien `atom:link rel="self"` dans chaque flux (recommandation du validateur W3C).
  L'URL publique de base est lue dans la configuration, pas écrite dans le code.
  Ajouter un test.
- Ces modifications se font **sur Windows**, avec les mêmes règles de qualité (§10) et un commit.
  Elles arrivent ensuite sur le VPS par `git push` (fait par l'utilisateur), puis `git pull`.

### 13.5 Plan de travail

Chaque phase se termine par une démonstration qui marche, un résumé, et l'accord de l'utilisateur.

**Phase H0 — Préparation**
- Générer une clé SSH `ed25519` sur Windows, avec OpenSSH intégré à Windows (`ssh-keygen`). *(Fait : `~\.ssh\id_ed25519`, protégée par une passphrase.)*
- L'utilisateur commande le VPS Cloud chez Infomaniak et installe Ubuntu LTS avec sa clé publique, depuis le Manager.
- Première connexion : `ssh <utilisateur>@<ip>`. L'utilisateur est `ubuntu` ou `root` selon l'image : voir le Manager.
- Démo : la connexion SSH fonctionne, sans mot de passe.

**Phase H1 — Sécuriser le serveur**
- Mettre le système à jour.
- Créer l'utilisateur admin avec `sudo` et sa clé.
- Interdire `root` et les mots de passe en SSH.
- Activer `ufw` et `unattended-upgrades`.
- Démo : connexion avec l'utilisateur admin ; `ssh root@<ip>` est refusé ; `sudo ufw status` affiche les ports 22, 80 et 443.

**Phase H2 — Rust et le programme sur le VPS**
- Installer les outils de compilation (`build-essential`, `pkg-config`, `libssl-dev` : sous Linux, `native-tls` utilise OpenSSL).
- Créer l'utilisateur `conv_rss` et installer Rust avec `rustup` pour lui.
- `git clone`, puis `cargo build --release`.
- Démo : un lancement manuel affiche le bilan, avec toutes les sources en `OK`.

**Phase H3 — Lien `atom:link` dans les flux**
- Modification du programme (§13.4) sur Windows, tests, commit, push, puis `git pull` et recompilation sur le VPS.
- Démo : `cargo test` passe, et les flux contiennent leur URL publique.

**Phase H4 — Publication avec Caddy**
- Installer Caddy.
- Écrire un `Caddyfile` qui sert uniquement le dossier des flux sur l'adresse sslip.io, avec le HTTPS automatique.
- Démo :
  - les flux s'ouvrent dans le navigateur en `https://` ;
  - le validateur W3C accepte l'URL sans recommandation ;
  - l'utilisateur s'abonne aux 5 flux dans son agrégateur.

**Phase H5 — Exécution quotidienne**
- Planifier le programme 1 fois par jour, sous l'utilisateur `conv_rss`.
- Présenter `cron` et les timers `systemd`, avec leurs avantages et leurs limites, puis choisir avec l'utilisateur.
- Enregistrer la sortie (le bilan) dans un fichier de log.
- Démo : un lancement forcé du planificateur produit les flux et une ligne dans le log. Le lendemain, on vérifie le log ensemble.

**Phase H6 — Mise à jour et maintenance**
- Écrire une procédure courte, en anglais, dans un `README` ou un fichier `docs/` :
  - mettre à jour le programme (`git pull`, `cargo build --release`) ;
  - lire les logs ;
  - vérifier que Caddy et le planificateur tournent ;
  - mettre à jour Rust (`rustup update`).
- Démo : l'utilisateur fait lui-même une mise à jour complète en suivant la procédure.

### 13.6 Décisions encore ouvertes (hébergement)

- Le dépôt GitHub est-il **public ou privé** ? S'il est privé, le VPS a besoin d'une « deploy key » en lecture seule pour faire `git clone`.
- **Heure** de l'exécution quotidienne et **fuseau horaire** du serveur.
- Comment être **prévenu d'une erreur** : simple lecture du log, ou alerte (e-mail…) plus tard ?
- **Sauvegardes** : option de l'hébergeur (snapshots), ou rien (le code est sur GitHub et les flux sont régénérés chaque jour).
- Ubuntu LTS ou Debian (Ubuntu proposé par défaut).
