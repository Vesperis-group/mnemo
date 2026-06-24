# CI/CD

Cette page résume les workflows GitHub Actions de mnemo. Tous épinglent leurs
actions tierces par SHA de commit complet, déclarent des permissions minimales
(`contents: read` par défaut) et figent leurs versions d'outillage.

## Workflows

| Workflow | Déclencheurs | Rôle |
| --- | --- | --- |
| `ci.yml` | push, pull_request | `fmt --check`, `clippy -D warnings`, tests, build. |
| `audit.yml` | push `main`, pull_request | `cargo-audit`, `cargo-deny`, `cargo-machete`, `gitleaks`. |
| `codeql.yml` | push, pull_request, schedule | Analyse statique (SAST) Rust. |
| `lint.yml` | push, pull_request | `actionlint` et `ShellCheck`. |
| `release.yml` | merge sur `main` | Release automatique signée (cosign, SBOM, provenance). |
| `release-smoke.yml` | release publiée, manuel, hebdomadaire | Smoke tests d'installation post-release. |

## `release-smoke.yml`

Ce workflow vérifie qu'une release **publiée** est réellement installable et
utilisable par un utilisateur final, en empruntant le chemin officiel
`scripts/install.sh`.

- **Déclencheurs** :
  - `release: [published]` — teste le tag qui vient d'être publié.
  - `workflow_dispatch` — teste un tag fourni en entrée (`version`), ou la
    dernière release si l'entrée est vide.
  - `schedule` (lundi 06:00 UTC) — revalide la dernière release publiée.
- **Permissions** : `contents: read` uniquement. Le workflow **ne publie rien**
  et n'utilise aucun jeton en écriture.
- **Résolution de version** : tag de l'évènement `release`, sinon entrée
  manuelle, sinon dernière release via l'API GitHub en lecture seule.
- **Jobs** :
  - `install-smoke` (glibc sur `ubuntu-22.04`, musl sur `ubuntu-24.04`) :
    installe la release dans un `HOME` temporaire et isolé
    (`MNEMO_ASSUME_YES=1`, `MNEMO_NO_BASHRC=1`), vérifie que la version
    installée correspond au tag, exécute les commandes principales
    (`init`, `doctor`, `completions`, `add`, `search`, `show`, `print`,
    `secrets scan`, `project list`), puis désinstalle proprement
    (`mnemo uninstall --yes --purge`). La sortie de `mnemo print` n'est jamais
    exécutée.
  - `asset-checksum-smoke` : télécharge les archives glibc et musl ainsi que
    `mnemo-<tag>-checksums.txt`, vérifie leur empreinte SHA-256, extrait
    chaque archive et exécute le binaire.

Ce workflow reste un **smoke test d'installation** : il ne duplique pas la
vérification complète de signature Sigstore et de provenance déjà effectuée
dans `release.yml`.
