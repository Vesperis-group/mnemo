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
| `scorecard.yml` | push `main`, règle de branche, hebdomadaire, manuel | OpenSSF Scorecard (posture sécurité open source). |
| `fuzz.yml` | pull_request, push `main`, hebdomadaire, manuel | Fuzzing `cargo-fuzz` (libFuzzer) des fonctions pures sensibles. |

Le dépôt utilise aussi [`.github/dependabot.yml`](../.github/dependabot.yml)
(écosystèmes `cargo`, `npm`, `github-actions`, cadence hebdomadaire) pour les
mises à jour de dépendances. Dependabot conserve l'épinglage des actions par SHA
et n'active aucun auto-merge. Voir [docs/SCORECARD.md](SCORECARD.md) pour la
posture OpenSSF Scorecard détaillée.

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

## `scorecard.yml`

Ce workflow exécute [OpenSSF Scorecard](https://scorecard.dev) pour mesurer la
posture de sécurité open source du dépôt (épinglage des actions, permissions des
workflows, politiques de branche, détection de secrets, etc.).

- **Déclencheurs** : `push` sur `main`, `branch_protection_rule`, `schedule`
  (lundi 07h00 UTC) et `workflow_dispatch`.
- **Permissions** : `contents: read` au niveau workflow. Le job ajoute seulement
  `id-token: write` (requis par `publish_results: true`) et
  `security-events: write` (remontée SARIF dans Code Scanning). Aucun jeton
  d'écriture sur le contenu, les actions, les packages, les issues ou les PR.
- **Publication** : `publish_results: true` alimente le badge public Scorecard
  (visible dans l'en-tête du README). Le SARIF est aussi remonté dans l'onglet
  Security (Code Scanning) et archivé en artefact.

Le workflow **ne publie aucune release** et ne modifie pas le produit. Ses
résultats aident à identifier les prochains durcissements de la chaîne
d'approvisionnement.

## `fuzz.yml`

Ce workflow exécute une baseline de fuzzing avec
[`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) (moteur libFuzzer) sur
des fonctions **pures** réellement sensibles, sans base de données, réseau ni
shell. Voir [docs/FUZZING.md](FUZZING.md) pour le détail des cibles.

- **Déclencheurs** : `pull_request` et `push` sur `main` (limités par `paths` à
  `src/**`, `fuzz/**`, `Cargo.toml`, `Cargo.lock` et le workflow lui-même),
  `schedule` (dimanche 05:00 UTC, campagne plus longue) et `workflow_dispatch`.
- **Permissions** : `contents: read` uniquement, aucun jeton en écriture.
- **Toolchain** : Rust **nightly** et `cargo-fuzz` (version épinglée) sont
  installés **uniquement dans ce workflow**. Le build, les tests et les releases
  normales restent sur la toolchain stable figée par `rust-toolchain.toml` ;
  nightly n'est jamais requis pour compiler ou utiliser `mnemo`.
- **Cibles** : `mdfmt_escape` (échappement Markdown), `secret_detection`
  (détection/redaction de secrets), `date_filter_parse` (parsing durées/dates).
  Durée courte par cible sur PR (30 s), plus longue sur `schedule` (120 s).
- **Corpus** : aucun corpus n'est téléchargé ni versionné ; les entrées
  intéressantes restent locales et ignorées par git (`fuzz/.gitignore`).
