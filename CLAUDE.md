# CLAUDE.md — Convertisseur RSS en Rust

## 1. Contexte

Ce projet fait partie d'une veille technologique basée sur des flux RSS et un agrégateur.
Certaines sources n'ont pas de flux RSS natif. Ce programme les convertit : il lit une page web,
repère la liste d'articles avec des sélecteurs CSS, et écrit un fichier RSS 2.0 par source.

Il remplace un service en ligne (FetchRSS) limité à 5 flux gratuits.
Contrainte : la solution doit rester **entièrement gratuite**.

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
- Rust **n'est pas encore installé** (voir phase 0).
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
link = "a"            # attribute href
date = "time"         # optional
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
- `.gitignore` : `target/` et `cache/`.
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

**Phase 6 — À décider plus tard (ne pas commencer sans demande)**
Publication des fichiers XML et exécution **1 fois par jour**.
Pistes à présenter le moment venu, avec avantages et limites :
GitHub Actions + GitHub Pages, ou Planificateur de tâches Windows + hébergement des fichiers.

## 12. Décisions encore ouvertes

- Où publier les fichiers XML (voir phase 6).
- Que faire des sources marquées `needs_js`.
- Ajout éventuel de Behance dans une v2.
