# CI/CD

Cette page résume les workflows GitHub Actions de mnemo. Tous épinglent leurs
actions tierces par SHA de commit complet, déclarent des permissions minimales
(`contents: read` par défaut) et figent leurs versions d'outillage.

## Workflows

| Workflow | Déclencheurs | Rôle |
| --- | --- | --- |
| `ci.yml` | push, pull_request | `fmt --check`, `clippy -D warnings`, tests, build ; smoke test `mnemo runbook`. |
| `audit.yml` | push `main`, pull_request | `cargo-audit`, `cargo-deny`, `cargo-machete`, `gitleaks`. |
| `codeql.yml` | push, pull_request, schedule | Analyse statique (SAST) Rust. |
| `lint.yml` | push, pull_request | `actionlint` et `ShellCheck`. |
| `release.yml` | merge sur `main` | Release automatique signée (cosign, SBOM, provenance SLSA, `.intoto.jsonl`). |
| `release-smoke.yml` | release publiée, manuel, hebdomadaire | Smoke tests d'installation post-release. |
| `publish-crates.yml` | release publiée, manuel | Publication `mnemo-rs` sur crates.io via OIDC Trusted Publishing. |
| `scorecard.yml` | push `main`, règle de branche, hebdomadaire, manuel | OpenSSF Scorecard (posture sécurité open source). |
| `fuzz.yml` | pull_request, push `main`, hebdomadaire, manuel | Fuzzing `cargo-fuzz` (libFuzzer) des fonctions pures sensibles. |

Le dépôt utilise aussi [`.github/dependabot.yml`](../.github/dependabot.yml)
(écosystèmes `cargo`, `npm`, `github-actions`, cadence hebdomadaire) pour les
mises à jour de dépendances. Dependabot conserve l'épinglage des actions par SHA
et n'active aucun auto-merge. Voir [docs/SCORECARD.md](SCORECARD.md) pour la
posture OpenSSF Scorecard détaillée.

## Publication crates.io

Le package crates.io est nommé `mnemo-rs` (binaire `mnemo`, lib interne `mnemo`).
La publication est automatisée via [`publish-crates.yml`](../.github/workflows/publish-crates.yml)
dès qu'une GitHub Release est publiée.

### Flow de publication

1. L'événement `release.published` déclenche `publish-crates.yml`.
2. Le workflow vérifie que le package s'appelle bien `mnemo-rs` et que la version n'est pas déjà présente sur crates.io.
3. `cargo publish --dry-run --locked` est exécuté en amont.
4. L'authentification crates.io utilise **OIDC Trusted Publishing** via l'action `rust-lang/crates-io-auth-action` : aucun token long terme `CRATES_IO_TOKEN` n'est stocké dans GitHub Secrets.
5. `cargo publish --locked` publie la version.

### Prérequis (setup manuel unique)

Côté crates.io - aller dans **Settings → Trusted Publishing → Add trusted publisher** pour `mnemo-rs` :

| Champ | Valeur |
| --- | --- |
| Repository owner | `Vesperis-group` |
| Repository name | `mnemo` |
| Workflow filename | `publish-crates.yml` |
| Environment | `crates-io` |

Côté GitHub - créer un **environment** nommé `crates-io` (Settings → Environments).
Le job `publish` utilise cet environment ; il est possible d'y ajouter des règles de protection (approbation manuelle, délai, etc.).

Aucun secret `CRATES_IO_TOKEN` n'est ajouté ni nécessaire. Le job demande `id-token: write` uniquement pour obtenir le token OIDC court terme ; toutes les autres permissions restent en lecture seule.

## `ci.yml` — job `runbook-smoke`

Le job `runbook-smoke` est intégré dans `ci.yml` aux côtés de `rust` et `scripts`.
Il tourne sur `ubuntu-24.04` avec `permissions: contents: read` uniquement et se
déclenche sur les mêmes triggers (`push`, `pull_request` sur `main`).

### Fonctionnement

1. **Build** : compile `mnemo` en mode release (`cargo build --locked --release`).
2. **Isolation** : initialise un `HOME` temporaire et isolé
   (`${{ runner.temp }}/mnemo-runbook-smoke`) puis exécute `mnemo init` pour
   créer une base SQLite propre.
3. **Injection** : ajoute trois commandes de test via `mnemo add --cmd … --exit-code 0`
   sous un `MNEMO_SESSION_ID` fixe (`smoke-runbook-ci`), ce qui les rattache à
   une session identifiable par `--last`.
4. **Tests smoke** :
   - `mnemo runbook --last` → sortie non vide.
   - La sortie contient `# Runbook`.
   - `mnemo runbook --last --output <fichier>` → le fichier est créé.
   - `mnemo runbook --last --limit 1` → la métadonnée `- Commands: 1` est présente.

### Couverture future (après PR2 — `feat/runbook-harden-exports`)

Une fois PR2 mergée, le job `runbook-smoke` sera étendu pour couvrir :
- `--format json` : la sortie est du JSON valide (vérification via `jq`).
- `--no-redact` : les commandes ne sont pas filtrées.
- `--group-by cwd` / `--group-by project` : les sections de groupement sont présentes.

## `release-smoke.yml`

Ce workflow vérifie qu'une release **publiée** est réellement installable et
utilisable par un utilisateur final, en empruntant le chemin officiel
`scripts/install.sh`.

- **Déclencheurs** :
  - `release: [published]` - teste le tag qui vient d'être publié.
  - `workflow_dispatch` - teste un tag fourni en entrée (`version`), ou la
    dernière release si l'entrée est vide.
  - `schedule` (lundi 06:00 UTC) - revalide la dernière release publiée.
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

## Provenance SLSA des releases (`.intoto.jsonl`)

Chaque release produit un fichier `mnemo-v<version>-provenance.intoto.jsonl`
attaché comme asset GitHub Release. Ce fichier est au format **in-toto JSONL**
(une enveloppe DSSE par ligne) et couvre les quatre artefacts principaux :
tarball glibc, tarball musl, SBOM CycloneDX et fichier de checksums.

### Différence entre les artefacts d'intégrité

| Artefact | Format | Rôle |
| --- | --- | --- |
| `<asset>.sha256` | texte | Empreinte SHA-256 de l'archive (vérification locale sans outil tiers). |
| `<asset>.sigstore.json` | Sigstore bundle | Signature cosign keyless (certificat Fulcio éphémère, entrée Rekor). |
| `<asset>.provenance.sigstore.json` | Sigstore bundle | Attestation de provenance SLSA v1 (prédicat `slsaprovenance1`, cosign). |
| `*-provenance.intoto.jsonl` | in-toto JSONL | Enveloppes DSSE extraites des bundles Sigstore ; format reconnu par SLSA et OpenSSF Scorecard. |
| `*-sbom.cdx.json` | CycloneDX | SBOM : liste des composants Rust du binaire. |
| `*-checksums.txt` | texte | Empreintes agrégées (archives + SBOM), vérifiées avant signature. |

### Génération

Le fichier `.intoto.jsonl` est produit par
[`scripts/intoto-provenance.sh`](../scripts/intoto-provenance.sh), appelé par
le hook `after:bump` de `release-it.json` **après** `scripts/sign-release.sh`.
Il extrait le champ `dsseEnvelope` de chaque bundle `.provenance.sigstore.json`
déjà produit et vérifié par cosign. Aucune signature supplémentaire n'est
effectuée ; le contenu cryptographique est identique aux bundles Sigstore.

Un step de validation dans `release.yml` vérifie, après la création de la
release, que l'asset `.intoto.jsonl` est bien présent dans la GitHub Release.



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

## Posture sécurité et OpenSSF Best Practices

L'ensemble de ces workflows constitue une **posture de sécurité** documentée et
sert de **preuves** pour le badge OpenSSF Best Practices :

- `ci.yml` - build, tests, format et lint sur chaque PR ;
- `codeql.yml` - analyse statique (SAST) ;
- `audit.yml` - `cargo-audit`, `cargo-deny`, `cargo-machete`, `gitleaks` ;
- `fuzz.yml` - fuzzing `cargo-fuzz` ;
- `release.yml` / `release-smoke.yml` - intégrité et vérification des releases ;
- `scorecard.yml` - suivi continu de la posture OpenSSF Scorecard.

Pour les opérations nécessitant une **traçabilité des commandes** (audits supply
chain, post-mortem d'incidents, documentation de release), la commande
`mnemo runbook` génère des runbooks Markdown ou JSON réutilisables depuis
l'historique local. Voir [docs/RUNBOOK.md](RUNBOOK.md) pour la référence
complète et les cas d'usage DevSecOps.

Le dossier de preuves est centralisé dans
[docs/OPENSSF_BEST_PRACTICES.md](OPENSSF_BEST_PRACTICES.md). Le projet a obtenu le
badge OpenSSF Best Practices **niveau Passing**
(<https://www.bestpractices.dev/projects/13366>), affiché dans le README.
