# Mnemo

[![CI](https://github.com/Vesperis-group/mnemo/actions/workflows/ci.yml/badge.svg)](https://github.com/Vesperis-group/mnemo/actions/workflows/ci.yml)
[![Audit](https://github.com/Vesperis-group/mnemo/actions/workflows/audit.yml/badge.svg)](https://github.com/Vesperis-group/mnemo/actions/workflows/audit.yml)
[![CodeQL](https://github.com/Vesperis-group/mnemo/actions/workflows/codeql.yml/badge.svg)](https://github.com/Vesperis-group/mnemo/actions/workflows/codeql.yml)
[![Release](https://github.com/Vesperis-group/mnemo/actions/workflows/release.yml/badge.svg)](https://github.com/Vesperis-group/mnemo/actions/workflows/release.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](#licence)
[![Rust 1.96](https://img.shields.io/badge/rust-1.96.0-orange.svg)](rust-toolchain.toml)
[![Supply chain](https://img.shields.io/badge/supply%20chain-SHA--256%20%2B%20cosign%20%2B%20SBOM-success.svg)](#sécurité-et-confidentialité)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/Vesperis-group/mnemo/badge)](https://scorecard.dev/viewer/?uri=github.com/Vesperis-group/mnemo)

https://github.com/user-attachments/assets/d7deb152-1e8d-48cd-8297-53a81798835d

```bash
curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh | bash
```

**Assistant d'historique shell local-first.** Recherche fuzzy, contexte projet
(Git), interface TUI, sauvegardes, maintenance et releases vérifiables. Un seul
binaire **Rust**, sans serveur ni cloud.

`mnemo` enregistre chaque commande exécutée dans une base **SQLite** locale, puis
la retrouve instantanément via une interface TUI « ops dashboard » ou en ligne de
commande. L'objectif est simple : un historique fiable, contextualisé et privé,
qui reste entièrement sur votre machine.

## Fonctionnalités

- 🦀 Rust, un seul binaire `mnemo` (~2,3 Mo), toolchain épinglée.
- 🔍 Recherche **fuzzy** interactive (`ratatui`, `crossterm`, `nucleo-matcher`).
- 🗄️ Stockage **SQLite** local (`rusqlite`), aucun réseau à l'usage.
- 🧭 Contexte **projet et branche Git** attaché à chaque commande.
- 🔒 Filtrage automatique des commandes sensibles, fichiers en `600`.
- 💾 Sauvegardes, restauration et maintenance par ancienneté intégrées.
- 🔏 Releases vérifiables : SHA-256, signatures cosign keyless, SBOM, provenance.
- 🐧 Cible principale : **Linux et WSL** avec Bash.
- 🤖 Mode non interactif `--print` (et sorties JSON) pour les scripts et la CI.
- 🚀 Onboarding guidé `mnemo init --wizard` et complétions shell `bash`, `zsh`, `fish`.
- 🗂️ Sessions de travail : `mnemo session list/show/export` (Markdown ou JSON).
- 🛡️ Nettoyage des secrets de l'historique : `mnemo secrets scan/redact` (dry-run par défaut).
- 🎯 Inspection et récupération sûres : `mnemo show <id>`, `mnemo print <id>` (jamais d'exécution automatique).

---

## Sommaire

- [Démarrage rapide](#démarrage-rapide)
- [Installation](#installation)
- [Désinstallation](#désinstallation)
- [Utilisation](#utilisation)
  - [Aperçu de la TUI](#aperçu-de-la-tui)
  - [Recherche avancée](#recherche-avancée)
  - [Statistiques](#statistiques)
  - [Gestion des données](#gestion-des-données)
  - [Maintenance automatique](#maintenance-automatique)
  - [Sessions de travail](#sessions-de-travail)
  - [Nettoyage des secrets](#nettoyage-des-secrets)
  - [Configuration](#configuration)
  - [Intégration Bash](#intégration-bash)
  - [Complétions shell](#complétions-shell)
  - [Page de manuel](#page-de-manuel)
  - [Diagnostic](#diagnostic)
  - [Version](#version)
  - [Mise à jour et cycle de vie](#mise-à-jour-et-cycle-de-vie)
- [Référence des commandes](#référence-des-commandes)
- [Sécurité et confidentialité](#sécurité-et-confidentialité)
- [Architecture](#architecture)
- [Compatibilité et stabilité](#compatibilité-et-stabilité)
- [Limites connues](#limites-connues)
- [Roadmap](#roadmap)
- [Dépannage](#dépannage)
- [Contribution](#contribution)
- [Licence](#licence)

---

## Démarrage rapide

Installer la dernière release, puis activer l'historique :

```bash
# 1. Installer (binaire musl statique vérifié par SHA-256)
curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh | bash

# 2. Premier démarrage guidé (intégration Bash, import, diagnostic)
mnemo init --wizard

# 3. Recharger le shell pour activer l'enregistrement et Ctrl+R
source ~/.bashrc

# 4. Ouvrir la recherche interactive
mnemo search
```

L'assistant `mnemo init --wizard` est entièrement non destructif : il propose
chaque étape avec une valeur par défaut sûre et n'efface jamais de données. Voir
[docs/UX_ONBOARDING.md](docs/UX_ONBOARDING.md) pour le détail du parcours.

Mode non interactif (scripts, CI), sans ouvrir la TUI :

```console
$ mnemo search cargo --print
cargo build --release
cargo test
cargo clippy
```

---

## Installation

### Installation distante (recommandée)

Installe la **dernière release** en téléchargeant un binaire pré-compilé (aucune
toolchain Rust requise). Par défaut, le binaire est **musl statique**, le plus
compatible.

```bash
curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh | bash
```

Le script effectue les étapes suivantes :

1. détecte l'architecture (`uname -m`) et choisit l'asset adapté ;
2. télécharge l'archive `.tar.gz` **et** son `.sha256` ;
3. **vérifie l'intégrité** (SHA-256, toujours obligatoire) avant toute
   installation ;
4. **vérifie la signature Sigstore** de l'archive si `cosign` est présent
   (best-effort par défaut, voir ci-dessous) ;
5. installe le binaire dans `~/.local/bin/mnemo` (créé si absent) ;
6. vérifie que `~/.local/bin` est dans le `PATH` ;
7. lance `mnemo init` ;
8. **propose** d'ajouter l'intégration Bash à `~/.bashrc` (sauvegarde puis
   protection anti-doublon) ;
9. affiche un résumé des prochaines étapes.

Choisir une **version précise** :

```bash
MNEMO_VERSION="v1.0.1" \
  bash <(curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh)
```

Choisir une **cible précise** (musl statique, ou GNU/glibc 2.35) :

```bash
MNEMO_TARGET="x86_64-unknown-linux-gnu-glibc2.35" \
  bash <(curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh)
```

Mode non interactif (utile en CI) :

```bash
MNEMO_ASSUME_YES=1 ... bash ...   # confirme automatiquement
MNEMO_NO_BASHRC=1  ... bash ...   # n'ajoute pas le bloc .bashrc
```

### Vérification de signature Sigstore

Le SHA-256 reste **toujours** obligatoire et bloquant. En complément (défense en
profondeur), le script vérifie aussi la **signature Sigstore** de l'archive
lorsque [`cosign`](https://docs.sigstore.dev/cosign/installation/) est installé :

- **Par défaut (best-effort)** : si `cosign` est absent ou si le bundle de
  signature est indisponible, le script **avertit** puis **continue**, car
  l'intégrité SHA-256 a déjà été vérifiée. Une signature présente mais
  **invalide** interrompt toujours l'installation.
- **Mode strict** : `MNEMO_REQUIRE_SIGNATURE=1` rend la vérification
  obligatoire. L'installation est **refusée** si `cosign` est absent, si le
  bundle est indisponible, ou si la signature est invalide.

```bash
# Installation strictement signée (refuse si cosign absent ou signature invalide)
MNEMO_REQUIRE_SIGNATURE=1 \
  bash <(curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh)
```

> `cosign` n'est **jamais** téléchargé automatiquement (pas de `curl | bash`
> implicite). Installez-le via le gestionnaire de paquets de votre distribution
> ou avec `go install github.com/sigstore/cosign/v2/cmd/cosign@latest`.
> L'identité et l'émetteur OIDC attendus sont configurables via
> `MNEMO_SIGN_IDENTITY` et `MNEMO_SIGN_OIDC_ISSUER`.

### Cibles Linux disponibles

| Asset | Cas d'usage |
| --- | --- |
| `x86_64-unknown-linux-musl` | **Recommandé.** Binaire **statique**, compatible avec quasiment toutes les distributions (aucune dépendance à la glibc du système). |
| `x86_64-unknown-linux-gnu-glibc2.35` | Binaire GNU construit sur Ubuntu 22.04, pour les environnements glibc **≥ 2.35**. |

> 🧱 Les variantes `aarch64-unknown-linux-*` pourront être ajoutées
> ultérieurement (cross-compilation).

### Installation depuis les sources

```bash
git clone https://github.com/Vesperis-group/mnemo.git
cd mnemo
bash scripts/install.sh                                # télécharge la release
MNEMO_INSTALL_FROM_SOURCE=1 bash scripts/install.sh    # compile localement
```

Le repli `MNEMO_INSTALL_FROM_SOURCE=1` compile depuis le dépôt cloné (ou clone
`MNEMO_REPO_URL` si les sources sont absentes) au lieu de télécharger un asset.

### Installation manuelle

Si vous préférez tout contrôler :

```bash
# 1. Compiler
cargo build --release

# 2. Installer le binaire
mkdir -p ~/.local/bin
install -m 0755 target/release/mnemo ~/.local/bin/mnemo

# 3. S'assurer que ~/.local/bin est dans le PATH (si nécessaire)
echo 'export PATH="$HOME/.local/bin:$PATH"' >> ~/.bashrc

# 4. Initialiser la config et la base
mnemo init

# 5. Copier le snippet d'intégration affiché par mnemo bashrc, le coller
#    dans ~/.bashrc, puis recharger le shell
mnemo bashrc
source ~/.bashrc
```

Avec le `Makefile` :

```bash
make release     # compilation optimisée
make install     # délègue à scripts/install.sh
```

---

## Désinstallation

mnemo gère lui-même son cycle de vie (voir [Mise à jour et cycle de
vie](#mise-à-jour-et-cycle-de-vie)). La commande intégrée est recommandée :

```bash
mnemo uninstall              # confirme, retire le binaire et le bloc .bashrc
mnemo uninstall --purge      # supprime aussi config, base et sauvegardes
```

Le script shell reste disponible pour les environnements sans binaire installé :

```bash
bash scripts/uninstall.sh
```

Ce script :

1. supprime `~/.local/bin/mnemo` s'il existe ;
2. **propose** de retirer le bloc mnemo de `~/.bashrc` (après sauvegarde) ;
3. **propose** de supprimer les données locales (`~/.config/mnemo`,
   `~/.local/share/mnemo`).

> 🔒 Les données ne sont **jamais** supprimées sans confirmation explicite.

Options du script :

```bash
MNEMO_ASSUME_YES=1 bash scripts/uninstall.sh   # confirme binaire et .bashrc, pas les données
MNEMO_PURGE=1      bash scripts/uninstall.sh   # supprime aussi les données
```

Ou via le `Makefile` :

```bash
make uninstall
```

---

## Utilisation

### Aperçu de la TUI

`mnemo tui` est l'interface interactive principale. `mnemo search` (sans
`--print`) utilise le même moteur ; `mnemo search --print` reste un mode non
interactif inchangé.

```bash
mnemo tui                        # toutes les commandes
mnemo tui cargo                  # requête initiale
mnemo tui --project mnemo        # filtre projet initial
mnemo tui --branch main          # filtre branche initial
mnemo tui --cwd /home/killian/mnemo
mnemo tui --failed               # uniquement les commandes en échec
```

#### Quatre zones (dashboard ops)

- **Barre de commande** (haut) : badges d'identité (`mnemo`, version, projet,
  branche, total), barre de recherche (frappe en direct), puces de filtres
  actifs (`[projet: …]`, `[branche: …]`, `[statut: échecs]`, `[dossier: …]` ou
  `[aucun filtre]`).
- **Synthèse / KPI** : `Total`, `Visibles`, `Succès`, `Échecs`, `Taux d'échec`,
  `Projets` et shell dominant (masquée sur terminal court). Hormis `Total`
  (ensemble chargé), tous les indicateurs portent sur les commandes **visibles**
  après filtres ; un échec est un `exit_code` différent de 0 et le taux vaut
  `Échecs / Visibles`.
- **Corps** : liste des commandes (heure, statut `✓` ou `✗`, contexte
  projet/dossier, commande tronquée) à gauche ; détails de la sélection à droite,
  organisés en sections **COMMAND / CONTEXT / EXECUTION / GIT / METADATA**
  (masqué sur terminal étroit).
- **Pied** : raccourcis essentiels et message de statut.

L'affichage est **responsive** : la synthèse et le panneau de détails se masquent
automatiquement sur les petits terminaux, sans jamais paniquer.

#### Deux contextes de saisie

- **Search** (par défaut) : la frappe édite la requête (recherche fuzzy en
  direct).
- **Details** : focus liste, raccourcis d'une lettre actifs (`j`/`k`, `/`,
  `x`/`d`, `r`, `f`, `F`, `p`, `b`, `y`/`c`, `e`, `?`, `q`).

`Tab` bascule entre les deux. `Esc` quitte partout. `F1` ouvre l'aide partout.

#### Raccourcis

| Touche | Action |
| --- | --- |
| *(saisie)* | filtre fuzzy en temps réel (mode recherche) |
| `↑` / `k`, `↓` / `j` | élément précédent / suivant |
| `PageUp` / `PageDown` | page précédente / suivante |
| `Home` / `End` | premier / dernier élément |
| `Entrée` | imprime la commande sélectionnée puis quitte |
| `Esc` / `q` | quitter |
| `Tab` | basculer le focus liste / détails |
| `/` | revenir au focus recherche (depuis la liste) |
| `r` | rafraîchir (recharge la base) |
| `y` / `c` | copier la commande (presse-papiers système si disponible, sinon tampon interne) |
| `e` | exporter les résultats filtrés en JSON (`mnemo-export-<ts>.json`) |
| `?` / `F1` | afficher / masquer l'aide |
| `x` / `d` | supprimer la commande sélectionnée |
| `y` / `n` (`Esc`) | confirmer / annuler la suppression |
| `f` | faire défiler le statut (tous, succès, échecs) |
| `p` / `b` | filtrer par le projet / la branche courant(e) |
| `F` | ouvrir / fermer le panneau de filtres |
| `Ctrl+P` / `Ctrl+B` / `Ctrl+D` | filtrer par projet / branche / dossier de la sélection |
| `Ctrl+L` | effacer tous les filtres |
| `Ctrl+C` | quitter (dans tous les modes) |

Dans le panneau de filtres (`F`) : `p`/`b`/`w` (projet/branche/dossier depuis la
sélection), `s` (statut : tous, succès, échecs), `c` (effacer).

> **Note :** `Ctrl+C` quitte toujours l'application, quel que soit le mode. Le
> filtre « par répertoire de la sélection » est sur `Ctrl+D`.

#### Copie sans dépendance graphique

La copie tente, dans l'ordre, `wl-copy`, `xclip`, puis `xsel`. Si aucun n'est
disponible (WSL minimal, environnement headless), un message l'indique et la
commande reste récupérable via `Entrée`. Aucune bibliothèque de presse-papiers
n'est liée au binaire.

#### Suppression sûre

`d` ouvre une confirmation : `Supprimer la commande #N ? Cette action créera
d'abord une sauvegarde.` Sur `y`, mnemo crée une **sauvegarde automatique**
(comme `mnemo delete`), supprime dans une transaction SQLite, retire l'entrée de
la liste et affiche un message de succès. **Si la sauvegarde échoue, rien n'est
supprimé.** `n` ou `Esc` annule.

#### États gérés

Base absente (propose `mnemo init`), base vide, recherche ou filtre sans
résultat, erreur SQLite (message propre), terminal trop petit (avertissement).

### Recherche avancée

`mnemo search --print` accepte des filtres **combinables** pour interroger
finement l'historique, sans TUI (idéal pour les scripts et la CI) :

```bash
mnemo search --print --failed                 # uniquement les commandes en échec
mnemo search --print --exit-code 127          # code de sortie exact
mnemo search docker --print --since 24h        # dernières 24 heures (durée)
mnemo search docker --print --since 7d        # 7 derniers jours (durée)
mnemo search --print --since 2026-01-01       # depuis une date (AAAA-MM-JJ)
mnemo search --print --before 2026-06-01      # avant une date (alias : --until)
mnemo search --print --cwd /home/killian/mnemo
mnemo search --print --shell bash
mnemo search --print --project current        # projet du dossier courant
mnemo search cargo --print --branch main --limit 50
mnemo search --json                           # sortie JSON stable (mode non interactif implicite)
mnemo search cargo --id-only                  # uniquement les IDs, un par ligne
```

- `--since` accepte une **durée** (`24h`, `7d`, `2w`, `3m`, `1y`) ou une **date**
  `AAAA-MM-JJ` ; `--before` (alias `--until`) attend une date. Une valeur de date
  **invalide** n'interrompt pas la commande : le filtre est simplement ignoré,
  avec un avertissement.
- `--failed` et `--exit-code` sont mutuellement exclusifs.
- `--project current` détecte automatiquement le projet du répertoire courant
  (racine Git, marqueur de projet, ou nom du dossier).
- `--json` produit un tableau d'objets au **format stable** (mêmes champs que
  `mnemo export --format json`) ; `--id-only` n'affiche que les identifiants.
  Ces deux options activent automatiquement le mode non interactif (inutile de
  préciser `--print`) et sont mutuellement exclusives.

Tous ces filtres sont également disponibles dans la **TUI** via les raccourcis
(`f` statut, `p`/`b` projet/branche courant, `Ctrl+P/B/D` depuis la sélection).

### Inspecter et récupérer une commande

`mnemo show` et `mnemo print` ciblent une commande par son ID (obtenu via
`mnemo list`, `mnemo search --id-only` ou la TUI). **mnemo n'exécute jamais une
commande de l'historique** : ces sous-commandes se contentent de lire la base.

```bash
mnemo show 128     # détail complet : date, dossier, contexte Git, session, code retour
mnemo print 128    # uniquement la commande brute sur stdout (copie/redirection)
```

- `mnemo show` omet les champs non renseignés plutôt que d'inventer une valeur.
- `mnemo print` n'émet aucun label ni couleur : juste la commande et un saut de
  ligne. Vous restez responsable de ce que vous faites de la sortie (la copier,
  la relire, éventuellement l'exécuter vous-même).
- Si une commande a déjà été nettoyée par `mnemo secrets redact`, c'est sa forme
  **redactée** stockée qui est affichée.
- Un ID inexistant produit une erreur claire sur stderr et un code de sortie non
  nul.

Détails et exemples supplémentaires dans [docs/SEARCH.md](docs/SEARCH.md).

### Statistiques

Chaque commande ajoutée via `mnemo add` est enrichie du contexte Git de son
répertoire (racine du dépôt, branche, remote `origin`) **lorsque disponible**.
Git reste **optionnel** : hors dépôt, ou si Git est absent, ces champs valent
`NULL` et rien ne change.

```bash
mnemo stats                              # statistiques d'usage (texte)
mnemo stats --project mnemo              # filtrées par projet
mnemo stats --branch main                # filtrées par branche
mnemo stats --project mnemo --branch main
mnemo stats --json                       # format machine (scripts / CI)
```

#### Normalisation des « Top commandes »

Le « Top commandes » compte le **programme réellement invoqué**, pas le premier
mot brut de la ligne. La normalisation :

- ignore les lignes vides, les commentaires (`# …`) et les tokens parasites
  (`-`, `|`, `&&`, `;`, `then`, `fi`, `done`, `function`) ;
- retire les affectations de variables en tête
  (`RUST_LOG=debug cargo test` devient `cargo`) ;
- traverse les wrappers `sudo`, `env`, `command`, `builtin`, `exec`, `time`,
  `nohup` (`sudo -E apt update` devient `apt`, `time cargo test` devient
  `cargo`) ;
- réduit les chemins au binaire (`/usr/bin/git status` devient `git`,
  `./target/release/mnemo doctor` devient `mnemo`).

Les entrées écartées sont comptées et affichées :
`Entrées ignorées dans le Top commandes : X`.

Exemple de sortie nettoyée :

```text
Top commandes :
      5  echo
      3  cargo
      2  apt
      2  git
      1  docker
      1  kubectl
      1  npm
      1  npx
  Entrées ignorées dans le Top commandes : 4
```

Exemple `mnemo stats --json` :

```json
{
  "total_commands": 1166,
  "git_projects": 1,
  "failed_commands": 0,
  "ignored_for_top_commands": 192,
  "ignored_commands_config": ["create_dir"],
  "filters": { "project": "mnemo", "branch": null },
  "top_commands": [
    { "name": "cargo", "count": 12 },
    { "name": "git", "count": 8 }
  ],
  "top_directories": [
    { "path": "/home/killian/mnemo", "count": 5 }
  ],
  "top_projects": [
    { "name": "mnemo", "count": 3 }
  ]
}
```

#### Ignorer des commandes dans les statistiques

Certaines commandes (helpers de scripts, alias internes) polluent le « Top
commandes » sans intérêt analytique. Vous pouvez les exclure **sans supprimer**
de données : elles restent en base et dans le total, mais sont comptées comme
« Entrées ignorées » au lieu d'apparaître dans le Top.

La liste vit dans `~/.config/mnemo/config.toml`, section `[stats]` :

```toml
[stats]
ignored_commands = ["create_dir", "export"]
```

La comparaison est **exacte et insensible à la casse**, appliquée au nom
*normalisé* de la commande (`create_dir foo` devient `create_dir`). Le filtre
n'agit que sur le « Top commandes » ; les totaux globaux restent inchangés.

Gérez la liste sans éditer le fichier à la main :

```bash
mnemo config stats-ignore add create_dir     # ajoute (idempotent, sans doublon)
mnemo config stats-ignore list               # affiche la liste
mnemo config stats-ignore remove create_dir  # retire
```

`mnemo doctor` affiche aussi un rappel des commandes ignorées
(`Commandes ignorées dans stats : create_dir, …`), et `mnemo stats --json`
expose la liste via le champ `ignored_commands_config`.

### Gestion des données

mnemo fournit des commandes sûres pour **sauvegarder, restaurer, exporter et
nettoyer** l'historique. Toutes les données restent **locales** (aucun cloud,
aucune synchronisation) : la base SQLite et la configuration ne quittent jamais
la machine.

#### Garanties de sécurité

Toute opération destructive (`delete`, `prune`, `restore`) respecte les mêmes
règles :

- **`--dry-run`** : affiche ce qui serait touché sans rien modifier ;
- **aperçu systématique** : la ou les commandes concernées sont affichées avant
  action ;
- **confirmation obligatoire** : sans `--yes`, une confirmation interactive est
  demandée. En mode **non interactif** (script, pipe), l'opération est
  **refusée** sans `--yes`, jamais de suppression silencieuse ;
- **sauvegarde automatique** : un backup complet est créé avant toute
  suppression ou restauration réelle ;
- **transactions SQLite** pour `delete`, `prune` et `restore`.

#### Sauvegarde et restauration

```bash
mnemo backup                       # archive dans ~/.local/share/mnemo/backups/
mnemo backup --output ~/backups    # dossier de destination personnalisé
mnemo backup --json                # sortie JSON (chemin et métadonnées)

mnemo restore ./mnemo-backup-YYYYMMDD-HHMMSS.tar.gz --dry-run
mnemo restore ./mnemo-backup-YYYYMMDD-HHMMSS.tar.gz          # confirmation interactive
mnemo restore ./mnemo-backup-YYYYMMDD-HHMMSS.tar.gz --yes    # sans question
```

Une sauvegarde est une archive `.tar.gz` autonome contenant `history.db`,
`config.toml` et un `metadata.json` (version mnemo, date ISO, chemins, taille de
la base, nombre de commandes, version de schéma). La restauration **valide**
l'archive (base ouvrable, table `commands`, version de schéma compatible) et
**crée d'abord un backup de l'état courant** avant de remplacer la base et la
config.

#### Export

```bash
mnemo export --format json
mnemo export --format csv
mnemo export --format json --output ./mnemo-export.json
mnemo export --project mnemo --format json
mnemo export --branch main --format csv
```

L'export JSON produit un tableau d'objets (tous les champs : `id`, `command`,
`cwd`, `shell`, `hostname`, `exit_code`, `created_at`, `git_root`, `git_branch`,
`git_remote`, `session_id`). L'export CSV respecte la RFC 4180 (échappement des
virgules, guillemets et sauts de ligne). Sans `--output`, l'export va sur
stdout. Les filtres `--project` et `--branch` s'appliquent comme pour `search`.

Les sorties sur stdout (`export`, `list`, `stats`) s'enchaînent avec les outils
Unix habituels sans déclencher d'erreur « Broken pipe » : mnemo s'arrête alors
silencieusement.

```bash
mnemo export --format json | head -20
mnemo export --format csv | head -5
mnemo export --format json | jq '.[0]'
mnemo list --limit 100 | less

# Conserver l'export complet dans un fichier, puis l'inspecter
mnemo export --format json --output mnemo-export.json
head -20 mnemo-export.json
```

#### Lister et supprimer

```bash
mnemo list                  # 20 dernières commandes avec leurs IDs
mnemo list --limit 20
mnemo list --project mnemo
mnemo list --branch main
mnemo list --json

mnemo delete 123 --dry-run  # montre la commande sans la supprimer
mnemo delete 123            # confirmation interactive et backup automatique
mnemo delete 123 --yes      # suppression directe
```

`mnemo list` affiche `id`, date courte, projet ou dossier, `exit_code` et la
commande, ce qui aide à repérer l'ID à passer à `mnemo delete`.

#### Nettoyage par ancienneté

```bash
mnemo prune --older-than 180d --dry-run
mnemo prune --older-than 30d --yes
mnemo prune --project mnemo --older-than 90d --dry-run
```

Durées acceptées : `30d` (jours), `12w` (semaines), `6m` (mois, environ 30
jours), `1y` (année, environ 365 jours). En `--dry-run`, mnemo affiche le nombre
d'entrées concernées et quelques exemples. Les filtres `--project` et `--branch`
sont respectés. Un backup automatique est créé avant toute suppression réelle.

### Maintenance automatique

mnemo peut **nettoyer périodiquement** les commandes anciennes, de façon
**opt-in** et toujours protégée. La configuration vit dans la section
`[maintenance]` de `~/.config/mnemo/config.toml` :

```toml
[maintenance]
auto_prune_enabled = false       # désactivé par défaut
auto_prune_after = "180d"        # ancienneté au-delà de laquelle purger
auto_backup_before_prune = true  # sauvegarde complète avant toute purge
```

```bash
mnemo maintenance status         # affiche la config et le nombre d'entrées éligibles
mnemo maintenance run --dry-run  # simule : montre ce qui serait supprimé, ne touche rien
mnemo maintenance run --yes      # applique la purge (sauvegarde d'abord si configuré)
```

Garanties :

- **Désactivé par défaut** : `mnemo maintenance run` ne supprime jamais rien
  tant que `auto_prune_enabled = false`.
- **Jamais de suppression silencieuse** : `run` exige `--yes` (ou une
  confirmation interactive) ; `--dry-run` ne modifie jamais la base.
- **Sauvegarde avant purge** : si `auto_backup_before_prune = true`, un backup
  complet est créé avant toute suppression réelle.
- `auto_prune_after` accepte les mêmes durées que `mnemo prune`
  (`30d`, `12w`, `6m`, `1y`).

### Sessions de travail

Une **session** regroupe les commandes d'un même shell interactif, identifiées
par `MNEMO_SESSION_ID`. L'intégration Bash (`mnemo init`) attribue
automatiquement cet identifiant à chaque shell. Les commandes importées d'un
ancien historique n'ont pas de session et ne sont donc pas listées.

```bash
mnemo session list                                  # sessions récentes
mnemo session show <session_id>                     # commandes d'une session
mnemo session export --last --output work-session.md
mnemo session export <session_id> --format json
```

L'export Markdown produit un document directement réutilisable (métadonnées,
bloc de commandes, tableau chronologique), utile pour documenter une
intervention, un TP, un audit ou une procédure. Un fichier de sortie existant
n'est jamais écrasé sans `--force`. Détails dans
[docs/SESSIONS.md](docs/SESSIONS.md).

> ℹ️ Une installation antérieure à `mnemo session` contient un bloc Bash qui ne
> capture pas encore `MNEMO_SESSION_ID`. Mettez-le à niveau sans réinstaller
> avec `mnemo shell upgrade` (sauvegarde automatique, bloc remplacé proprement),
> puis rechargez le shell avec `source ~/.bashrc`. `mnemo doctor` signale aussi
> ce cas et propose la commande.

### Activité par projet

`mnemo project` regroupe l'historique par **projet** (racine Git, `git_root`)
pour naviguer par dépôt et produire des rapports d'activité réutilisables.

```bash
mnemo project list                                  # projets connus (commandes, sessions, dernière activité, branches)
mnemo project list --json --limit 10                # même inventaire, en JSON
mnemo project show mnemo                             # détail d'un projet (par nom ou racine)
mnemo project show --current                         # projet du dossier courant
mnemo project report --current --since 30d           # rapport Markdown des 30 derniers jours
mnemo project report mnemo --format json --output rapport.json
```

`project show` affiche les métadonnées du projet (racine, remote, nombre de
commandes et de sessions, période d'activité, branches), ses commandes récentes
et ses derniers échecs. `project report` génère un document directement
réutilisable (Markdown par défaut ou JSON) : agrégats de la période, détail
chronologique et liste des échecs, filtrable par `--since`/`--until`. Comme le
reste de mnemo, ces commandes sont en **lecture seule** : aucune commande de
l'historique n'est jamais exécutée, et les commandes déjà redactées le restent.
Un fichier de sortie existant n'est jamais écrasé sans `--force`. Détails dans
[docs/PROJECTS.md](docs/PROJECTS.md).


### Nettoyage des secrets

À l'enregistrement, `mnemo` ignore déjà les commandes sensibles (voir
`sensitive_keywords`). `mnemo secrets` traite l'historique **déjà stocké**, par
exemple importé d'un `~/.bash_history` antérieur à l'installation.

```bash
mnemo secrets scan                 # liste les commandes suspectes (toujours redactées)
mnemo secrets scan --json          # même résultat en JSON, sans aucune valeur en clair
mnemo secrets redact               # dry-run : montre ce qui serait nettoyé, ne modifie rien
mnemo secrets redact --apply --yes # redacte en base après sauvegarde automatique
```

Garanties :

- Aucune valeur sensible n'est jamais affichée en clair, ni en texte ni en JSON.
  En cas de doute, la commande entière devient `[REDACTED COMMAND]`.
- `redact` est en dry-run par défaut ; `--apply` est requis pour écrire.
- Une sauvegarde complète est créée **avant** toute écriture ; si elle échoue,
  rien n'est modifié.
- Seule la colonne `command` est réécrite ; horodatage, dossier, code de sortie
  et contexte Git sont conservés.

La détection est heuristique (jetons `Bearer`, affectations `PASSWORD=`/`TOKEN=`,
URLs `user:motdepasse@hôte`, options `--password`/`--token`, mot de passe attaché
des clients SQL, fragments de clé privée) : protection raisonnable, pas
exhaustivité. Détails dans [docs/SECRETS.md](docs/SECRETS.md).

### Configuration

```bash
mnemo config show        # affiche la configuration effective (TOML)
mnemo config path        # chemin du fichier config.toml
mnemo config edit        # ouvre $EDITOR (repli nano ou vi), avec sauvegarde préalable
mnemo config validate    # valide la config (code 1 si erreur)
```

`mnemo config edit` crée une configuration par défaut si elle est absente,
**sauvegarde** systématiquement l'ancienne version
(`config.toml.bak.AAAAMMJJ-HHMMSS`) avant d'ouvrir l'éditeur, puis **revalide**
le résultat. `mnemo config validate` signale les erreurs (par exemple
`search_limit = 0`, ou `auto_prune_after` illisible) et les avertissements (clés
inconnues). La configuration n'est **jamais** écrasée sans sauvegarde.

Les chemins suivent la spécification **XDG Base Directory** (via la crate
`dirs`) :

| Donnée | Variable XDG | Chemin par défaut |
| --- | --- | --- |
| Configuration | `$XDG_CONFIG_HOME` | `~/.config/mnemo/config.toml` |
| Base SQLite | `$XDG_DATA_HOME` | `~/.local/share/mnemo/history.db` |
| Binaire | *(aucune)* | `~/.local/bin/mnemo` |

Exemple de `config.toml` :

```toml
sensitive_keywords = [
    "password", "passwd", "token", "secret",
    "api_key", "bearer", "private_key", "sshpass",
]
ignore_prefixes = ["mnemo"]
search_limit = 5000
```

- `sensitive_keywords` : une commande contenant l'un de ces mots (insensible à
  la casse) n'est **pas** enregistrée.
- `ignore_prefixes` : préfixes de commandes à ne jamais enregistrer.
- `search_limit` : nombre maximal de commandes chargées dans la TUI.

### Intégration Bash

`mnemo init` (ou `mnemo bashrc`) fournit le bloc à coller dans `~/.bashrc`.
`scripts/install.sh` peut l'ajouter automatiquement, encadré par :

```bash
# >>> mnemo init >>>
...   # snippet généré par `mnemo bashrc`
# <<< mnemo init <<<
```

Le snippet :

- branche `__mnemo_record` sur `PROMPT_COMMAND` pour enregistrer chaque commande
  (avec son code de sortie et le répertoire courant) ;
- **n'enregistre jamais** la commande `mnemo` elle-même ;
- remappe `Ctrl+R` pour ouvrir la recherche TUI et insérer la commande choisie.
  La TUI s'affiche sur `/dev/tty`, donc elle fonctionne même dans une
  substitution `$(mnemo search)`.

Après ajout :

```bash
source ~/.bashrc
mnemo import
mnemo search
```

### Complétions shell

`mnemo completions <shell>` écrit le script de complétion sur la sortie standard
pour `bash`, `zsh` ou `fish`. mnemo **ne modifie jamais** vos fichiers shell :
vous redirigez vous-même la sortie.

```bash
# Bash, pour la session courante
source <(mnemo completions bash)

# Installation persistante
mnemo completions bash > ~/.local/share/bash-completion/completions/mnemo
mnemo completions zsh  > ~/.zsh/completions/_mnemo
mnemo completions fish > ~/.config/fish/completions/mnemo.fish
```

Un shell non supporté produit une erreur claire listant les valeurs possibles.

### Page de manuel

Une page de manuel au format troff est fournie dans le dépôt :

```bash
man ./docs/man/mnemo.1
```

### Diagnostic

`mnemo doctor` inspecte l'installation locale et affiche un rapport clair. En
mode simple, **il ne modifie jamais le système**.

```bash
mnemo doctor          # diagnostic (lecture seule)
mnemo doctor --fix    # répare les éléments manquants (non destructif)
mnemo doctor --json   # sortie JSON exploitable (scripts / CI)
```

#### Contrôles effectués

- Binaire `mnemo` trouvable dans le `PATH` (et chemin détecté) et version.
- `~/.local/bin` présent dans le `PATH`.
- Présence de `~/.config/mnemo/config.toml` et `~/.local/share/mnemo/history.db`.
- Base SQLite ouvrable, table `commands` présente, nombre de commandes.
- Présence de `~/.bashrc`, du bloc mnemo (non dupliqué) et du bind `Ctrl+R`.
- Shell courant (`$SHELL`), avec un avertissement si ce n'est pas Bash.
- `HISTTIMEFORMAT`, avec une information si la variable n'est pas configurée.
- Permissions des fichiers sensibles : la config, la base et les sauvegardes
  doivent être privées (`600`). Toute permission plus ouverte (`644`, `664`) est
  signalée en `WARN`. Les archives de sauvegarde trop ouvertes sont rapportées de
  façon **agrégée** (un seul résumé, sans lister chaque fichier) :
  `[WARN ] Backups trop ouverts : 9 fichier(s), attendu 600`.

#### Statuts et code retour

Chaque ligne porte un statut `[OK]`, `[WARN]`, `[ERROR]`, `[INFO]` ou `[FIX]`.

| Code retour | Signification |
| --- | --- |
| `0` | Tout est OK ou seulement des avertissements. |
| `1` | Au moins une **erreur bloquante** (par exemple base corrompue, table absente). |

#### Mode `--fix`

`mnemo doctor --fix` répare l'installation de façon **non destructive** :

- crée les dossiers de configuration et de données s'ils sont absents ;
- crée la config si absente ;
- crée la base si absente ;
- resserre les permissions trop ouvertes de la config et de la base à `600`
  (lecture / écriture propriétaire uniquement, `[FIX]`) ;
- resserre à `600` les **archives de sauvegarde existantes** trop ouvertes, avec
  un résumé unique `[FIX  ] Permissions corrigées : 9 backup(s) → 600` (le
  contenu des archives n'est jamais modifié, aucune archive n'est supprimée) ;
- ajoute le bloc mnemo au `.bashrc` si absent, **supprime les doublons** et
  **restaure le raccourci `Ctrl+R`** s'il a disparu (toujours avec sauvegarde du
  `.bashrc` avant modification) ;
- affiche un message clair si `~/.local/bin` n'est pas dans le `PATH` ;
- **ne supprime jamais** de données.

À la fin, un résumé indique le nombre de corrections appliquées
(`Corrections appliquées : X`) ou `Aucune correction nécessaire`.

#### Exemple de sortie

```text
mnemo doctor : rapport de diagnostic
------------------------------------
[INFO ] mnemo version 1.0.1
[ OK  ] Binaire trouvé dans le PATH : ~/.local/bin/mnemo
[ OK  ] ~/.local/bin est dans le PATH
[ OK  ] Configuration présente : ~/.config/mnemo/config.toml
[ OK  ] Permissions correctes (600)
[ OK  ] Base présente : ~/.local/share/mnemo/history.db
[ OK  ] Table `commands` présente
[INFO ] 128 commande(s) enregistrée(s)
[ OK  ] ~/.bashrc présent
[ OK  ] Bloc d'intégration mnemo présent
[ OK  ] Bloc mnemo unique
[ OK  ] Raccourci Ctrl+R configuré
[ OK  ] Shell courant : /bin/bash
[INFO ] HISTTIMEFORMAT non configuré : les horodatages d'import seront approximatifs
------------------------------------
Résumé : 11 OK, 0 WARN, 0 ERROR, 2 INFO, 0 FIX
État global : sain (code 0)
```

Sortie JSON (`--json`) :

```json
{
  "summary": { "ok": 11, "warn": 0, "error": 0, "info": 2, "fix": 0, "exit_code": 0 },
  "checks": [
    { "name": "binary.version", "status": "info", "message": "mnemo version 1.0.1" },
    { "name": "db.table", "status": "ok", "message": "Table `commands` présente" }
  ]
}
```

Le JSON est produit via `serde_json` (sérialisation robuste, échappement correct
des caractères spéciaux).

### Version

La commande `mnemo version` donne un aperçu complet du binaire en cours
d'exécution, pratique pour les rapports de bug et la vérification d'installation :

```console
$ mnemo version
mnemo 1.0.1
  cible   : linux/x86_64
  profil  : release
  binaire : /home/<user>/.local/bin/mnemo
```

| Champ | Source |
| --- | --- |
| version | `CARGO_PKG_VERSION` (champ `version` du `Cargo.toml`) |
| cible | `std::env::consts::OS` et `std::env::consts::ARCH` |
| profil | `debug` ou `release` (`cfg!(debug_assertions)`) |
| binaire | `std::env::current_exe()` |

### Mise à jour et cycle de vie

mnemo gère lui-même son cycle de vie, sans dépendre des scripts shell.

#### `mnemo update` : y a-t-il du nouveau ?

```bash
mnemo update                 # compare version installée et dernière release
mnemo update --json          # sortie machine (vérification seule)
mnemo update --upgrade       # enchaîne l'upgrade si une mise à jour existe
mnemo update --upgrade --yes # idem, sans confirmation (automatisation)
```

Interroge l'API GitHub Releases (pré-releases ignorées) et affiche la version
installée, la dernière version et si une mise à jour est disponible.

**En terminal interactif**, lorsqu'une mise à jour est disponible, `mnemo update`
propose de l'installer immédiatement :

```text
Mise à jour disponible ✓
Installer maintenant avec `mnemo upgrade` ? [o/N]
```

La réponse par défaut (`Entrée`) est **non** ; répondre `o`, `oui`, `y` ou `yes`
enchaîne directement `mnemo upgrade` (vérification SHA-256, sauvegarde et
remplacement atomique, sans seconde confirmation). **En mode non interactif**
(CI, script, cron, pipe) ou avec `--json`, `update` reste une simple vérification
et n'installe **rien** : il se contente d'indiquer `Lancez \`mnemo upgrade\` pour
l'installer.`

L'option `--upgrade` lance l'installation sans poser la question de `update` ;
sans `--yes`, c'est `mnemo upgrade` qui demande sa confirmation finale (un seul
prompt). `--upgrade --yes` permet un upgrade entièrement automatisé. Aucune
installation n'a lieu sans consentement. Le drapeau `--require-signature` est
transmis tel quel à `mnemo upgrade`.

Exemple JSON :

```json
{
  "current_version": "v1.0.0",
  "latest_version": "v1.0.1",
  "update_available": true,
  "asset_target": "x86_64-unknown-linux-musl"
}
```

#### `mnemo upgrade` : installer la dernière version

```bash
mnemo upgrade                 # dernière version stable (confirmation demandée)
mnemo upgrade --yes           # sans question
mnemo upgrade --dry-run       # montre ce qui serait fait, n'installe rien
mnemo upgrade --version v1.0.1 # version précise
mnemo upgrade --target aarch64-unknown-linux-musl
mnemo upgrade --require-signature # exige une signature Sigstore valide
```

Déroulé : téléchargement de l'archive **et** de son `.sha256`, **vérification
SHA-256 avant extraction** (toujours obligatoire), **vérification de la signature
Sigstore** lorsque `cosign` est présent, contrôle que le nouveau binaire répond,
sauvegarde automatique des données, puis remplacement **atomique** de
`~/.local/bin/mnemo`.

La signature Sigstore suit la même logique que `install.sh` : best-effort par
défaut (avertissement si `cosign` est absent ou si le bundle est indisponible,
l'intégrité SHA-256 ayant déjà été vérifiée), **strict** avec
`--require-signature` (l'upgrade est refusé si la signature ne peut pas être
vérifiée). Une signature présente mais invalide refuse **toujours** l'upgrade.

> 🔒 `upgrade` ne touche **jamais** à `history.db`, `config.toml` ni aux
> sauvegardes. HTTPS et SHA-256 sont obligatoires ; aucun script distant n'est
> exécuté ; `cosign` n'est jamais téléchargé automatiquement. En cas d'échec, le
> binaire en place reste intact.

#### `mnemo uninstall` : retirer mnemo

```bash
mnemo uninstall              # demande confirmation, puis retire binaire et bloc .bashrc
mnemo uninstall --yes        # sans question (CI / script), conserve les données
mnemo uninstall --dry-run    # aperçu sans rien modifier
mnemo uninstall --purge      # supprime aussi config, base et sauvegardes (confirmation forte)
mnemo uninstall --purge --yes
```

`uninstall` est **toujours protégé par une confirmation** : retirer le binaire et
l'intégration shell reste une action destructive. En interactif, une question est
posée (`Désinstaller mnemo tout en conservant les données ? [y/N]`) ; en mode non
interactif **sans `--yes`**, la commande refuse proprement avec un code de sortie
non nul et le message *« Confirmation requise. Relancez avec --yes pour confirmer
ou --dry-run pour prévisualiser. »*

Par défaut, `uninstall` retire le binaire et le bloc d'intégration `.bashrc`
(après sauvegarde) mais **conserve toutes les données**. Avec `--purge`, une
sauvegarde de sécurité est créée hors du dossier de données, puis config, base et
sauvegardes sont supprimées **après confirmation forte** ; en mode non
interactif, `--purge` exige aussi `--yes`.

> `--dry-run` ne supprime **jamais** rien, et aucune donnée n'est touchée sans
> `--purge`.

---

## Référence des commandes

| Commande | Description |
| --- | --- |
| `mnemo init` | Crée `~/.config/mnemo/config.toml`, `~/.local/share/mnemo/history.db` et affiche le snippet `.bashrc`. |
| `mnemo init --wizard [--yes]` | Assistant d'onboarding interactif et non destructif (intégration Bash, import, diagnostic). `--yes` accepte les choix sûrs par défaut en contexte non interactif. |
| `mnemo completions <bash\|zsh\|fish>` | Écrit un script de complétion shell sur stdout (n'écrit jamais dans vos fichiers shell). |
| `mnemo import [--file <chemin>]` | Importe `~/.bash_history` (ou un fichier donné) dans SQLite. |
| `mnemo add --cmd "<cmd>" [--cwd "<dir>"] [--exit-code <n>]` | Ajoute une commande dans la base. |
| `mnemo tui [requête] [--project <nom>] [--branch <branche>] [--cwd <chemin>] [--failed]` | Ouvre la **TUI avancée** (recherche, filtres, détails, suppression). |
| `mnemo search [requête]` | Ouvre la même TUI interactive ; la commande choisie est imprimée sur stdout. |
| `mnemo search <requête> --print [--limit N]` | **Mode non interactif** : imprime les résultats sur stdout, sans TUI. |
| `mnemo search --query <requête> --print` | Variante avec option explicite `--query`. |
| `mnemo search <requête> [--project <nom>] [--branch <branche>] [--exit-code <n>] [--failed] [--since <durée\|date>] [--before <date>] [--cwd <chemin>] [--shell <shell>] [--limit N] [--json] [--id-only]` | **Recherche avancée** : filtres combinables par contexte Git, code de sortie, date (`--since` accepte `24h`/`7d`/…, `--before` alias `--until`), répertoire, shell ; sortie JSON stable avec `--json`, IDs seuls avec `--id-only` (mode non interactif implicite). |
| `mnemo show <id>` | Affiche le détail complet d'une commande (date, dossier, contexte Git, session, code retour, commande). Lecture seule : n'exécute **jamais** la commande. |
| `mnemo print <id>` | Imprime uniquement la commande brute sur stdout (sans label ni couleur). N'exécute **jamais** la commande ; ID inexistant : erreur sur stderr, code non nul. |
| `mnemo bashrc` | Affiche uniquement le snippet d'intégration Bash. |
| `mnemo shell upgrade` | Met à niveau le bloc d'intégration Bash existant vers la version courante (capture de `MNEMO_SESSION_ID` pour `mnemo session`) : sauvegarde automatique, bloc remplacé proprement, reste du `~/.bashrc` intact. |
| `mnemo migrate` | Applique les migrations de schéma SQLite en attente (idempotent, non destructif). |
| `mnemo stats [--project <nom>] [--branch <branche>] [--since <durée\|date>] [--json]` | Statistiques d'usage enrichies (totaux, taux d'échec, top commandes / dossiers / projets / shells, activité quotidienne), filtrables (dont `--project current`) et exportables en JSON. |
| `mnemo project <current\|list>` | Affiche le projet courant (racine Git, marqueur de projet ou dossier) ou la liste des projets connus (`list [--limit N] [--json]` : commandes, sessions, dernière activité, branches). |
| `mnemo project show <projet\|--current> [--limit N] [--json]` | Détaille un projet : métadonnées (racine, remote, commandes, sessions, période, branches), commandes récentes et derniers échecs. Lecture seule : n'exécute **jamais** les commandes. |
| `mnemo project report <projet\|--current> [--since <durée\|date>] [--until <date>] [--format markdown\|json] [--output <fichier>] [--force] [--limit N]` | Génère un rapport d'activité réutilisable (Markdown par défaut ou JSON) : agrégats de la période, détail chronologique et échecs ; stdout par défaut, jamais d'écrasement sans `--force`. |
| `mnemo maintenance <status\|run>` | État du nettoyage automatique ; `run --dry-run` simule, `run --yes` applique (désactivé par défaut, sauvegarde avant purge). |
| `mnemo session list [--limit N]` | Liste les sessions de travail (groupées par `session_id`), de la plus récente à la plus ancienne. |
| `mnemo session show <session_id> [--limit N]` | Affiche les commandes d'une session dans l'ordre chronologique. |
| `mnemo session export [<session_id>\|--last] [--format markdown\|json] [--output <fichier>] [--force]` | Exporte une session en Markdown (défaut) ou JSON ; stdout par défaut, jamais d'écrasement sans `--force`. |
| `mnemo secrets scan [--limit N] [--json]` | Repère dans l'historique stocké les commandes potentiellement sensibles, toujours affichées redactées (lecture seule). |
| `mnemo secrets redact [--apply] [--yes] [--backup]` | Redacte en place les commandes sensibles ; dry-run par défaut, sauvegarde obligatoire avant écriture, seule la colonne `command` est modifiée. |
| `mnemo config <show\|path\|edit\|validate>` | Affiche, localise, édite (`$EDITOR`, sauvegarde automatique) ou valide la configuration. |
| `mnemo config stats-ignore <add\|remove\|list> [<cmd>]` | Gère les commandes exclues du « Top commandes » dans `mnemo stats`. |
| `mnemo list [--limit N] [--project <nom>] [--branch <branche>] [--json]` | Affiche les dernières commandes avec leurs IDs (utile pour `mnemo delete`). |
| `mnemo backup [--output <dossier>] [--json]` | Crée une sauvegarde locale complète (`.tar.gz`). |
| `mnemo restore <archive> [--dry-run] [--yes]` | Restaure une sauvegarde après vérification, avec backup de sécurité. |
| `mnemo export --format <json\|csv> [--project <nom>] [--branch <branche>] [--output <fichier>] [--gzip]` | Exporte les commandes (stdout par défaut) ; `--gzip` produit un `.json.gz` ou `.csv.gz`. |
| `mnemo delete <id> [--dry-run] [--yes]` | Supprime une commande par ID (confirmation et backup automatique). |
| `mnemo prune --older-than <durée> [--project <nom>] [--branch <branche>] [--dry-run] [--yes]` | Nettoie les commandes anciennes (`30d`, `12w`, `6m`, `1y`). |
| `mnemo doctor [--fix] [--json]` | Diagnostique l'installation et, avec `--fix`, répare les éléments manquants. |
| `mnemo version` | Affiche la version, la cible (OS / arch), le profil de build et le chemin du binaire. |
| `mnemo update [--json] [--upgrade] [--yes] [--require-signature]` | Vérifie si une nouvelle version est disponible. En terminal interactif, propose l'installation immédiate ; `--upgrade` enchaîne `mnemo upgrade` (avec `--yes` pour l'automatisation, `--require-signature` transmis tel quel). Sans terminal ou avec `--json`, n'installe **rien**. |
| `mnemo upgrade [--dry-run] [--yes] [--version <vX.Y.Z>] [--target <triplet>] [--require-signature]` | Télécharge et installe la dernière version (vérification SHA-256 obligatoire, signature Sigstore best-effort ou stricte via `--require-signature`, remplacement atomique). |
| `mnemo uninstall [--dry-run] [--yes] [--purge]` | Désinstalle mnemo. **Conserve les données** sauf `--purge`. |

Sans `--print`, les commandes de recherche gardent le comportement TUI **par
défaut**.

---

## Sécurité et confidentialité

- **Local-first.** Vos données d'historique ne quittent jamais la machine : pas
  de serveur, pas de synchronisation cloud. Les seules connexions réseau
  possibles sont **explicites et déclenchées par vous** : `mnemo update` et
  `mnemo upgrade` contactent l'API GitHub en HTTPS pour vérifier ou télécharger
  une release. `mnemo doctor` est **hors-ligne par défaut**.
- **Téléchargements vérifiés.** `mnemo upgrade` télécharge l'archive **et** son
  empreinte `SHA-256`, vérifie la correspondance **avant** extraction, puis
  décompresse via un extracteur durci contre le *path traversal* (voir
  [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md)). Aucun script distant n'est
  jamais exécuté.
- **Filtrage des secrets.** Toute commande contenant `password`, `passwd`,
  `token`, `secret`, `api_key`, `bearer`, `private_key` ou `sshpass` est ignorée
  à l'import comme à l'ajout. La liste est personnalisable.
- **Auto-exclusion.** Les commandes commençant par `mnemo` ne sont pas
  enregistrées (cela évite de polluer l'historique).
- **Pas de modification destructive sans sauvegarde.** Les scripts
  d'installation et de désinstallation sauvegardent `~/.bashrc` avant toute
  modification (`~/.bashrc.mnemo.bak.YYYYMMDD-HHMMSS`) et ne suppriment jamais les
  données sans confirmation. La restauration crée une sauvegarde de sûreté avant
  de remplacer la base.
- **Permissions.** La config, la base et les sauvegardes restent dans votre
  répertoire utilisateur et sont créées en `600` (lecture / écriture
  propriétaire uniquement) sous Unix ; les dossiers gérés par mnemo sont en
  `700`. `mnemo doctor` signale toute permission trop ouverte et
  `mnemo doctor --fix` la resserre automatiquement à `600`.

> ℹ️ Le filtrage par mots-clés est une protection « best-effort », pas une
> garantie absolue. Vérifiez votre historique si vous manipulez des secrets.

Pour signaler une vulnérabilité, suivez la procédure décrite dans
[SECURITY.md](SECURITY.md).

### DevSecOps et chaîne d'approvisionnement

mnemo est outillé comme un vrai projet DevSecOps :

- **Invariants et threat model** documentés dans
  [docs/INVARIANTS.md](docs/INVARIANTS.md) (garanties testées) et
  [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md) (actifs, menaces M1 à M9,
  mitigations, risques résiduels).
- **Portes de qualité locales** :

  ```bash
  make check          # fmt --check + clippy -D warnings + tests
  make audit          # cargo audit / deny / machete + gitleaks (si installés)
  make sast           # clippy + shellcheck + actionlint
  make security-full  # porte stricte : audit supply chain + ShellCheck + actionlint
  make sbom           # génère le SBOM CycloneDX (cargo-cyclonedx épinglé)
  make sign-check     # vérifie l'outillage de signature / provenance (sans signer)
  make release-check  # porte complète : lint + tests + build release + musl
                      # + bash -n scripts + release-it --dry-run
  ```

- **CI/CD** : `ci.yml` (fmt/clippy/test/build, permissions en lecture seule),
  `audit.yml` (cargo-audit, cargo-deny, cargo-machete, gitleaks sur PR et push
  main), `codeql.yml` (SAST Rust), `lint.yml` (actionlint et ShellCheck),
  `release.yml` (release automatique au merge, permissions scopées),
  `release-smoke.yml` (smoke tests d'installation post-release, lecture seule,
  ne publie rien — voir [docs/CI_CD.md](docs/CI_CD.md)),
  `fuzz.yml` (baseline `cargo-fuzz` des fonctions pures sensibles — Markdown,
  détection de secrets, parsing de durées/dates ; nightly requis uniquement pour
  le fuzzing, voir [docs/FUZZING.md](docs/FUZZING.md)).
- **Politique des dépendances** dans [deny.toml](deny.toml) : licences
  permissives autorisées uniquement, refus des vulnérabilités RustSec, contrôle
  des sources. `RUSTSEC-2024-0436` (`paste`) est accepté temporairement car
  transitif via `ratatui` (advisory `unmaintained`, pas une vulnérabilité
  active) ; il est suivi pour suppression lors d'une mise à jour future de
  Ratatui.
- **Chaîne de release durcie** :
  - Release **bloquée** si la qualité (`fmt`/`clippy`/`tests`/`build`) ou
    l'audit (`cargo deny`/`cargo audit`/`gitleaks`) échoue : le job `publish`
    déclare `needs: [quality, audit]` et `if: success()`.
  - Assets publiés avec leur **checksum SHA-256** (`.tar.gz` et `.tar.gz.sha256`,
    glibc et musl), vérifié au packaging (`sha256sum -c`).
  - **SBOM CycloneDX** (`*-sbom.cdx.json`) généré par `cargo-cyclonedx` (version
    épinglée) et attaché à chaque release, avec son `.sha256`.
  - **Checksums agrégés** (`*-checksums.txt`) couvrant les deux archives et le
    SBOM, vérifiés avant signature.
  - **Signatures et provenance keyless** : chaque artefact est signé par
    `cosign` (version épinglée, OIDC ambiant GitHub Actions, **aucun secret long
    terme**) et accompagné d'une **attestation de provenance SLSA v1**. Les
    bundles Sigstore (`*.sigstore.json` et `*.provenance.sigstore.json`) sont
    produits **et vérifiés** dans les hooks `after:bump` de release-it, **avant**
    la création de la release : toute défaillance de signature, de provenance ou
    de SBOM **avorte la release** (aucune publication).
  - Actions GitHub **épinglées par SHA** de commit ; binaires `gitleaks` et
    `cosign` vérifiés par SHA-256 avant exécution (pas de `curl | bash`).
  - **Versions d'outillage figées** (aucun canal flottant) : Rust épinglé par
    [rust-toolchain.toml](rust-toolchain.toml) (`1.96.0` avec `rustfmt`, `clippy`
    et la cible musl, lu par le `rustup` du runner, sans action tierce de
    toolchain) ; Node.js épinglé par [.node-version](.node-version) (`24.15.0`,
    via `node-version-file`) ; outils Cargo (`cargo-audit`, `cargo-deny`,
    `cargo-machete`, `cargo-cyclonedx`) installés en **version exacte**
    (`--version … --locked`).
  - **Runners épinglés** : `ubuntu-24.04` (et `ubuntu-22.04` pour l'asset GNU lié
    à la glibc 2.35), jamais `ubuntu-latest`.
  - **Lockfiles obligatoires** (`Cargo.lock`, `package-lock.json`) ; CI en
    `cargo … --locked` et `npm ci` (pas de mise à jour implicite).
  - **Permissions minimales** : `contents: read` partout, `contents: write`
    uniquement dans le job de publication, plus `id-token: write` (OIDC keyless
    cosign) limité à ce même job.

  Détails complets dans [docs/THREAT_MODEL.md](docs/THREAT_MODEL.md), section
  « Durcissement CI/CD et chaîne de release ».

### Vérifier l'intégrité d'une release

Chaque release publie, pour chaque artefact `<asset>` : `<asset>.sha256`
(empreinte), `<asset>.sigstore.json` (signature cosign) et
`<asset>.provenance.sigstore.json` (attestation de provenance SLSA v1). Le
fichier `mnemo-v<version>-checksums.txt` agrège les empreintes.

```bash
# 1. Empreinte SHA-256 (toujours disponible, aucun outil tiers requis)
sha256sum -c mnemo-v<version>-x86_64-unknown-linux-musl.tar.gz.sha256

# 2. Signature cosign keyless (nécessite cosign installé)
cosign verify-blob \
  --bundle mnemo-v<version>-x86_64-unknown-linux-musl.tar.gz.sigstore.json \
  --certificate-identity-regexp '^https://github.com/Vesperis-group/mnemo/\.github/workflows/.+@refs/heads/main$' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  mnemo-v<version>-x86_64-unknown-linux-musl.tar.gz

# 3. Provenance SLSA v1 (attestation)
cosign verify-blob-attestation \
  --bundle mnemo-v<version>-x86_64-unknown-linux-musl.tar.gz.provenance.sigstore.json \
  --type slsaprovenance1 --check-claims=true \
  --certificate-identity-regexp '^https://github.com/Vesperis-group/mnemo/\.github/workflows/.+@refs/heads/main$' \
  --certificate-oidc-issuer 'https://token.actions.githubusercontent.com' \
  mnemo-v<version>-x86_64-unknown-linux-musl.tar.gz
```

> `install.sh` et `mnemo upgrade` vérifient **systématiquement** l'empreinte
> SHA-256 avant toute extraction ou remplacement de binaire. Ils vérifient
> **aussi automatiquement la signature Sigstore** de l'archive lorsque `cosign`
> est présent : best-effort par défaut (avertissement si `cosign` est absent,
> l'intégrité SHA-256 étant déjà garantie), strict avec
> `mnemo upgrade --require-signature` ou `MNEMO_REQUIRE_SIGNATURE=1` pour
> `install.sh`. La vérification de la **provenance SLSA** reste **manuelle**
> (commande ci-dessus) : seule la signature de l'archive est contrôlée
> automatiquement.

---

## Architecture

### Modules

```text
src/
├── main.rs        # point d'entrée et dispatch CLI
├── cli.rs         # définitions Clap (sous-commandes et options)
├── config.rs      # chemins XDG, TOML, durcissement des permissions (600)
├── db.rs          # schéma SQLite, insert, recherche filtrée, hash
├── migrations.rs  # migrations de schéma versionnées
├── importer.rs    # import de ~/.bash_history
├── filter.rs      # détection des commandes sensibles
├── gitctx.rs      # détection du contexte projet et branche Git
├── project.rs     # nom de projet courant et projets connus
├── stats.rs       # statistiques d'usage (texte et JSON)
├── export.rs      # export JSON (et gzip) des résultats
├── backup.rs      # sauvegardes horodatées (création centralisée)
├── archive.rs     # lecture et écriture des archives de sauvegarde
├── prune.rs       # nettoyage par ancienneté
├── maintenance.rs # maintenance automatique (status / run / dry-run)
├── list.rs        # listing et suppression d'entrées
├── show.rs        # détail (mnemo show) et récupération brute (mnemo print)
├── confirm.rs     # confirmations interactives sûres
├── lifecycle.rs   # update / upgrade / uninstall (avec lifecycle/)
├── doctor.rs      # diagnostic de l'installation (mnemo doctor)
├── version.rs     # informations de version et build
├── shell.rs       # génération du snippet Bash et helpers .bashrc
└── tui/           # interface Ratatui (dashboard ops)
    ├── ui.rs       # rendu (barre de commande, synthèse, liste, détails)
    ├── theme.rs    # palette et styles centralisés
    ├── format.rs   # helpers de formatage purs et testables
    ├── app.rs      # modèle et logique (navigation, filtres, KPI)
    ├── events.rs   # mapping clavier vers action
    ├── actions.rs  # actions et accès base isolé (trait)
    ├── help.rs     # texte d'aide
    └── clipboard.rs # copie système optionnelle
scripts/
├── install.sh         # installation (locale ou distante)
├── uninstall.sh       # désinstallation
├── package-release.sh # construction de l'archive de release
├── generate-sbom.sh   # SBOM CycloneDX (cargo-cyclonedx)
├── checksums-release.sh # empreintes SHA-256 agrégées des assets
├── sign-release.sh    # signatures et provenance cosign (keyless, vérifiées)
└── lib/bashrc.sh      # logique .bashrc partagée (et testée)
```

### Architecture locale

```mermaid
flowchart LR
    Shell([Bash + hook]) -->|mnemo add| Filter["filter.rs<br/>(commandes sensibles)"]
    Git["gitctx.rs<br/>(projet / branche)"] --> Filter
    Filter --> DB["db.rs"]
    Config["config.rs<br/>(TOML, 600)"] --> DB
    DB --> SQLite[("SQLite history.db")]
    SQLite --> Search["search / tui.rs"]
    SQLite --> Stats["stats.rs"]
    SQLite --> Export["export.rs"]
    SQLite --> Backup["backup.rs"]
    Search --> User([Utilisateur])
    Stats --> User
    Export --> User
```

### Flux : enregistrement d'une commande Bash vers SQLite

```mermaid
sequenceDiagram
    participant B as Bash (PROMPT_COMMAND)
    participant H as __mnemo_record
    participant M as mnemo add
    participant F as filter.rs
    participant D as db.rs
    participant S as SQLite

    B->>H: apres chaque commande
    H->>H: recupere cmd + exit code + PWD
    alt commande "mnemo*"
        H-->>B: ignoree
    else
        H->>M: mnemo add --cmd --cwd --exit-code
        M->>F: is_sensitive(cmd) ?
        alt sensible
            F-->>M: oui, abandon
        else
            F-->>M: non
            M->>D: insert_command (hash FNV-1a)
            D->>S: INSERT OR IGNORE
        end
    end
```

### Flux : import de `~/.bash_history`

```mermaid
flowchart TD
    Start([mnemo import]) --> Read["Lire ~/.bash_history"]
    Read --> Loop{Pour chaque ligne}
    Loop -->|vide ou #timestamp| Skip[Ignorer]
    Loop -->|sensible| SkipSec[Ignorer + compteur secrets]
    Loop -->|doublon hash| SkipDup[Ignorer + compteur doublons]
    Loop -->|nouvelle| Insert["INSERT OR IGNORE dans SQLite"]
    Insert --> Loop
    Skip --> Loop
    SkipSec --> Loop
    SkipDup --> Loop
    Loop -->|fin| Stats["Afficher statistiques (total / importees / secrets / doublons)"]
    Stats --> End([Termine])
```

### Flux : recherche TUI

```mermaid
sequenceDiagram
    participant U as Utilisateur
    participant T as tui.rs
    participant N as nucleo-matcher
    participant D as db.rs

    U->>T: mnemo search
    T->>D: fetch_all(limit)
    D-->>T: commandes recentes
    loop saisie / navigation
        U->>T: frappe une touche
        T->>N: fuzzy_filter(query)
        N-->>T: indices tries par score
        T-->>U: rendu sur /dev/tty
    end
    U->>T: Entree
    T-->>U: commande selectionnee sur stdout
```

### Schéma simplifié de la base SQLite

```mermaid
erDiagram
    COMMANDS {
        INTEGER id PK
        TEXT    command "NOT NULL"
        TEXT    cwd
        TEXT    shell
        TEXT    hostname
        INTEGER exit_code
        TEXT    created_at "NOT NULL"
        TEXT    git_root
        TEXT    git_branch
        TEXT    git_remote
        TEXT    session_id
        TEXT    hash "UNIQUE"
    }
```

Le dédoublonnage repose sur `hash` (FNV-1a 64 bits sur `command` et `cwd`) avec
contrainte `UNIQUE` et `INSERT OR IGNORE`. Les colonnes de contexte Git et de
session sont ajoutées par des migrations versionnées (voir `migrations.rs`).

### Cycle d'installation

```mermaid
flowchart LR
    A([scripts/install.sh]) --> B["cargo build --release"]
    B --> C["Installer dans ~/.local/bin"]
    C --> D{~/.local/bin dans PATH ?}
    D -->|non| D2["Avertir l'utilisateur"]
    D -->|oui| E
    D2 --> E["mnemo init"]
    E --> F{Ajouter le bloc .bashrc ?}
    F -->|oui| G["Sauvegarde + ajout (anti-doublon)"]
    F -->|non| H
    G --> H["Resume final"]
    H --> I([source ~/.bashrc])
```

### Cycle de vie : `update` et `upgrade`

```mermaid
sequenceDiagram
    participant U as Utilisateur
    participant M as mnemo update/upgrade
    participant R as GitHub Releases
    participant FS as ~/.local/bin

    U->>M: mnemo update
    M->>R: dernière version publiée ?
    R-->>M: version + assets
    alt déjà à jour
        M-->>U: rien à faire
    else nouvelle version
        M-->>U: propose la mise à jour
        U->>M: mnemo upgrade
        M->>R: télécharge binaire + SHA-256 (+ bundle cosign)
        M->>M: vérifie SHA-256 (et signature si demandé)
        alt vérification OK
            M->>FS: remplace le binaire atomiquement
            M-->>U: installé
        else vérification KO
            M-->>U: refuse, ne remplace rien (exit non nul)
        end
    end
```

### Chaîne d'approvisionnement de release

```mermaid
flowchart TD
    Tag([Tag de version]) --> CI["Workflow release.yml"]
    CI --> Build["cargo build --release --locked"]
    Build --> Pkg["Archive de release"]
    Pkg --> Sha["SHA-256 des assets"]
    Pkg --> Sbom["SBOM CycloneDX"]
    Pkg --> Sign["Signatures cosign keyless"]
    Sign --> Prov["Provenance SLSA"]
    Sha --> Pub["Publication GitHub Release"]
    Sbom --> Pub
    Prov --> Pub
    Pub --> Verify{Vérification côté client}
    Verify -->|SHA-256| Ok([Installation autorisée])
    Verify -->|cosign strict| Ok
    Verify -->|échec| Refus([Installation refusée])
```

### Cycle de vie des données

```mermaid
flowchart LR
    Cmd([Commande exécutée]) --> Add["mnemo add"]
    Add --> DB[("SQLite history.db")]
    DB --> Search["recherche / TUI"]
    DB --> Backup["mnemo backup"]
    Backup --> Arch[("Archives horodatées")]
    Arch --> Restore["mnemo restore"]
    Restore --> DB
    DB --> Prune["maintenance / prune<br/>(ancienneté)"]
    Prune -->|sauvegarde préalable| Arch
    Prune --> DB
    DB --> Export["mnemo export (JSON / gzip)"]
```

### Flux : diagnostic `mnemo doctor`

```mermaid
flowchart TD
    Start([mnemo doctor]) --> Fix{--fix ?}
    Fix -->|oui| Repair["Creer config / base si absentes<br/>Ajouter bloc .bashrc (sauvegarde, sans doublon)"]
    Fix -->|non| Checks
    Repair --> Checks["Controles lecture seule"]
    Checks --> C1["PATH + binaire + version"]
    Checks --> C2["config.toml + history.db"]
    Checks --> C3["SQLite ouvrable + table commands + count"]
    Checks --> C4[".bashrc : bloc + doublon + Ctrl+R"]
    Checks --> C5["shell + HISTTIMEFORMAT + permissions"]
    C1 --> Out{--json ?}
    C2 --> Out
    C3 --> Out
    C4 --> Out
    C5 --> Out
    Out -->|oui| J["Sortie JSON"]
    Out -->|non| T["Rapport texte (OK/WARN/ERROR/INFO)"]
    J --> Code{Erreur bloquante ?}
    T --> Code
    Code -->|oui| E1([exit 1])
    Code -->|non| E0([exit 0])
```

---

## Compatibilité et stabilité

| Élément | État |
| --- | --- |
| **OS principal** | Linux (x86-64), y compris **WSL2** |
| **Shell** | Bash (hook d'enregistrement `PROMPT_COMMAND`) |
| **Binaire** | statique `x86_64-unknown-linux-musl` pour les releases |
| **macOS** | compile depuis les sources, **non testé en continu** |
| **Windows natif** | non visé (utiliser WSL) |
| **Zsh / Fish** | recherche utilisable, hook d'enregistrement non fourni |

mnemo est conçu et validé pour **Linux et WSL avec Bash**. Les autres
environnements peuvent fonctionner mais ne bénéficient pas de la CI ni de
garanties. Le détail figure dans [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md).

À partir de la **v1.0**, mnemo suit le **versionnage sémantique** :

- changement incompatible (commande ou option retirée, format de sortie cassé)
  donne une version **majeure** ;
- nouvelle commande ou option rétrocompatible donne une version **mineure** ;
- correction de bug ou de sécurité sans rupture donne une version **corrective**.

Garanties visées à partir de la v1.0 :

- **Base de données** : migrée automatiquement vers le schéma courant ; mnemo
  **refuse** une base créée par une version plus récente plutôt que de la
  corrompre.
- **Sorties JSON** (`search --json`, `stats --json`, `doctor --json`, `export`) :
  structure stable et versionnée ; les ajouts se font par champs additionnels.
- **Codes de sortie** : `0` succès, `0` pour `doctor` sain ou avec avertissements
  seulement, `1` si `doctor` détecte une erreur, code non nul pour les erreurs
  CLI, une configuration invalide ou une vérification de signature stricte en
  échec.

Détails complets : [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md).

---

## Limites connues

- Pas d'horodatage par commande à l'import : toutes les lignes de
  `.bash_history` reçoivent l'heure de l'import (Bash ne stocke les dates que si
  `HISTTIMEFORMAT` est actif).
- Recherche fuzzy en mémoire (chargement jusqu'à `search_limit`, 5000 par
  défaut), sans pagination côté base.
- Bash uniquement (le hook d'enregistrement ne couvre pas encore Zsh ni Fish) ;
  pas de recherche plein-texte SQLite (FTS).
- Hash de dédoublonnage non cryptographique (FNV-1a), adapté au dédoublonnage,
  pas à la sécurité.
- Cible validée : Linux et WSL. macOS et Windows ne sont pas testés en continu
  (voir [Compatibilité et stabilité](#compatibilité-et-stabilité)).

---

## Roadmap

Voir aussi [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) pour les garanties de
stabilité visées à partir de la v1.0.

- [x] Commande `mnemo doctor` (diagnostic, `--fix`, `--json`).
- [x] Filtres TUI : par projet, branche, répertoire, statut (succès et échecs).
- [x] Commande `mnemo stats` (texte et JSON).
- [x] Suppression d'entrées (avec sauvegarde) et export JSON ou gzip.
- [x] Sauvegardes, restauration et maintenance par ancienneté.
- [x] TUI « ops dashboard » (synthèse / KPI, détails sectionnés, palette).
- [ ] Timestamps réels par commande (capture dans le hook Bash).
- [ ] Aperçu multi-lignes et coloration syntaxique dans la TUI.
- [ ] Support Zsh et Fish.
- [ ] FTS5 SQLite pour de très gros historiques.
- [ ] Chiffrement optionnel de la base (toujours local).

---

## Dépannage

> 💡 En cas de doute, lancez d'abord `mnemo doctor` : il identifie la plupart des
> problèmes ci-dessous, et `mnemo doctor --fix` en répare beaucoup
> automatiquement (sans rien supprimer).

**`mnemo: command not found` après installation**
`~/.local/bin` n'est pas dans le `PATH`. Ajoutez à `~/.bashrc` :
```bash
export PATH="$HOME/.local/bin:$PATH"
```
puis `source ~/.bashrc`.

**`version GLIBC_2.39 not found` (Ubuntu 22.04, WSL, distribution plus ancienne)**
Le binaire GNU est lié à la glibc de la machine de build. Un binaire construit sur
une distribution récente exige une glibc récente. Solutions :
- utilisez une release **`v1.0.1`** ou plus récente ;
- préférez l'asset **`x86_64-unknown-linux-musl`** (statique, sans dépendance à la
  glibc), qui est le **choix par défaut** de l'installateur :
  ```bash
  MNEMO_TARGET="x86_64-unknown-linux-musl" \
    bash <(curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh)
  ```
- si vous tenez au binaire GNU, prenez `x86_64-unknown-linux-gnu-glibc2.35`
  (construit sur Ubuntu 22.04, compatible glibc ≥ 2.35).

**`Aucune commande enregistrée. Lancez mnemo import d'abord.`**
La base est vide. Lancez `mnemo import` ou exécutez quelques commandes après avoir
activé l'intégration Bash.

**Les nouvelles commandes ne sont pas enregistrées**
Vérifiez que le bloc mnemo est présent dans `~/.bashrc` (`mnemo bashrc` pour le
voir) et que vous avez rechargé le shell (`source ~/.bashrc`). Vérifiez aussi que
`PROMPT_COMMAND` contient `__mnemo_record` :
```bash
echo "$PROMPT_COMMAND"
```

**`Ctrl+R` n'ouvre pas mnemo**
Le `bind -x` nécessite un shell interactif. Assurez-vous que le bloc est chargé et
qu'aucun autre outil (fzf, Atuin) ne capture déjà `Ctrl+R` après mnemo.

**La TUI ne s'affiche pas ou `/dev/tty` est indisponible**
La recherche interactive requiert un vrai terminal. En CI ou via un pipe, utilisez
le mode non interactif : `mnemo search <requête> --print`.

**Une commande sensible a été enregistrée**
Ajoutez le mot-clé manquant dans `sensitive_keywords` de
`~/.config/mnemo/config.toml`, puis ré-importez si besoin.

**Erreur de compilation liée à SQLite**
La crate `rusqlite` est compilée avec SQLite embarqué (`bundled`). Un compilateur
C est requis (`build-essential` sous Debian ou Ubuntu).

**Diagnostiquer rapidement l'état de l'installation**
```bash
mnemo doctor          # rapport lisible
mnemo doctor --json   # pour un script
mnemo doctor --fix    # répare config / base / bloc .bashrc (non destructif)
```
Un code retour `1` indique une erreur bloquante (par exemple base corrompue) ; `0`
signifie sain ou simples avertissements.

---

## Contribution

Les contributions sont les bienvenues. Le développement suit une règle simple :
toute modification passe par une **branche dédiée** puis une **Pull Request**, le
push direct sur `main` étant interdit par convention.

Le guide complet du contributeur est dans [CONTRIBUTING.md](CONTRIBUTING.md). Le
projet maintient par ailleurs un dossier de preuves
[docs/OPENSSF_BEST_PRACTICES.md](docs/OPENSSF_BEST_PRACTICES.md) pour préparer une
future demande de badge OpenSSF Best Practices (badge non encore obtenu).

### Mettre en place l'environnement

```bash
git clone https://github.com/Vesperis-group/mnemo.git
cd mnemo
make build      # cargo build
make test       # cargo test (unitaires + intégration scripts / CLI)
```

La toolchain Rust est épinglée dans [rust-toolchain.toml](rust-toolchain.toml)
(`1.96.0`) et lue automatiquement par `rustup`.

### Cibles de développement

```bash
make build      # cargo build
make release    # cargo build --release
make test       # cargo test
make lint       # cargo fmt --check + clippy -D warnings
make fmt        # cargo fmt
make check      # fmt --check + clippy + tests (avant commit)
make audit      # cargo audit / deny / machete + gitleaks (si installés)
make sast       # clippy + shellcheck + actionlint
make security-full  # porte stricte : audit supply chain + ShellCheck + actionlint
make release-check  # porte de qualité complète avant release
make clean      # cargo clean
make help       # liste toutes les cibles
```

### Avant d'ouvrir une PR

Lancez la porte de qualité locale (elle reproduit la CI) :

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
cargo build --release
bash -n scripts/install.sh
bash -n scripts/uninstall.sh
```

Un hook [`pre-commit`](https://pre-commit.com/) **optionnel** est fourni
(`.pre-commit-config.yaml`). Il n'est pas imposé. Activation manuelle :

```bash
pipx install pre-commit
pre-commit install
pre-commit run --all-files
```

### Couverture des tests

- filtre des secrets (`filter`) ;
- hash, dédoublonnage, insertion et horodatage SQLite (`db`) ;
- import `.bash_history` (`importer`) ;
- mode `--print` non interactif (`tui` et `tests/cli.rs`) ;
- diagnostic `doctor` : HOME sain, config ou base absente, base corrompue,
  `--fix`, `--json`, codes retour (`tests/doctor.rs`) ;
- syntaxe des scripts (`bash -n`), idempotence et sauvegarde du `.bashrc`
  (`tests/scripts.rs`) ;
- extraction d'archives durcie contre le *path traversal* (`archive`,
  `tests/v3_data_management.rs`, `tests/v5_lifecycle.rs`).

### Politique de branche

Le développement direct sur `main` est interdit par convention. Le workflow
`.github/workflows/branch-policy.yml` fournit un garde-fou best-effort, mais la
vraie protection est activée côté GitHub via les rulesets. Détails :
[docs/REPOSITORY_RULES.md](docs/REPOSITORY_RULES.md).

> ⚠️ La GitHub App `vesperis-mnemo-release` pousse le commit et le tag de
> release ; elle (et elle seule) doit figurer dans la *bypass list* des rulesets
> `main` et `v*`, sinon le push de release échouera. Voir
> [docs/RELEASE_APP.md](docs/RELEASE_APP.md).

### Release

Le projet publie via [`release-it`](https://github.com/release-it/release-it),
déclenché par un **push sur `main`** (typiquement le merge d'une PR).
`Cargo.toml` reste la **source de vérité** de la version Rust ; l'incrément est
calculé à partir des *Conventional Commits*. Procédure et durcissement complets :
[docs/RELEASE_APP.md](docs/RELEASE_APP.md) et
[docs/THREAT_MODEL.md](docs/THREAT_MODEL.md).

Simulation locale, sans rien publier :

```bash
npm ci
npm run release:dry
```

---

## Licence

Distribué sous licence **MIT**. Voir [LICENSE](LICENSE).
