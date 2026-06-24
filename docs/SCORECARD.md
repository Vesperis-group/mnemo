# OpenSSF Scorecard

Cette page documente la posture [OpenSSF Scorecard](https://scorecard.dev) du
dépôt : score courant, checks faibles, corrections appliquées et limites
assumées. Le workflow d'évaluation est décrit dans [docs/CI_CD.md](CI_CD.md)
(`scorecard.yml`).

## Score courant

- **Score global** : 7,6 / 10 (relevé du 2026-06-24, commit `2bfd37c`).
- Source : `https://api.scorecard.dev/projects/github.com/Vesperis-group/mnemo`.

## Diagnostic par check

| Check | Score | Raison | Action | Priorité |
| --- | ---: | --- | --- | --- |
| CI-Tests | 10 | Tests exécutés sur les PR (18/18) | — | — |
| SAST | 10 | CodeQL actif sur tous les commits | — | — |
| Pinned-Dependencies | 10 | Actions épinglées par SHA | — | — |
| Dangerous-Workflow | 10 | Aucun motif dangereux | — | — |
| Binary-Artifacts | 10 | Aucun binaire commité | — | — |
| Security-Policy | 10 | `SECURITY.md` présent | — | — |
| License | 10 | Licence MIT détectée | — | — |
| Token-Permissions | 10 | Permissions au moindre privilège | `security-events: write` scopé au job `analyze` | A (fait) |
| Dependency-Update-Tool | 10 | Dependabot détecté | `.github/dependabot.yml` ajouté | A (fait) |
| Fuzzing | 10 | Baseline `cargo-fuzz` détectée (3 cibles) | `fuzz/` + `fuzz.yml` ajoutés | A (fait) |
| Signed-Releases | 8 | Provenance sur le fichier de checksums, pas sur chaque artefact | Logique release (non modifiée ici) | C |
| Vulnerabilities | 8 | `RUSTSEC-2026-0002` (lru) et `RUSTSEC-2024-0436` (paste), transitifs via `ratatui` | Suivi, autorisés dans `deny.toml` | B |
| Branch-Protection | 8 | Protection non maximale (non appliquée aux admins, 1 approbation requise) | Rulesets (non modifiés ici) | C |
| Code-Review | 1 | 3/23 changesets approuvés | Nécessite revue + approbation systématiques | C |
| CII-Best-Practices | 0 | Badge OpenSSF Best Practices niveau Passing obtenu (projet 13366) | Le check passe au vert au prochain scan Scorecard | A (obtenu) |
| Maintained | 0 | Projet créé il y a moins de 90 jours | S'améliore avec le temps | — |
| Contributors | 0 | 0 organisation contributrice | Hors de portée d'une PR (pas de faux contributeurs) | — |
| Packaging | -1 | Aucun workflow de packaging reconnu | Non concluant (publication binaire via GitHub App) | — |

## Corrections appliquées

### Dependency-Update-Tool (0 → 10)

Ajout de [`.github/dependabot.yml`](../.github/dependabot.yml) couvrant les
écosystèmes `cargo`, `npm` et `github-actions`, en cadence hebdomadaire, avec des
groupes minor/patch pour limiter le bruit et des labels explicites.

Dependabot **conserve l'épinglage par SHA** des actions GitHub : il met à jour le
SHA et le commentaire de version, sans réintroduire de tag flottant. Aucun
auto-merge n'est configuré : chaque PR passe par la CI, l'audit et la revue.

### Token-Permissions (9 → 10 attendu)

Le workflow `codeql.yml` déclarait `security-events: write` au niveau du
workflow. Cette permission est désormais accordée **uniquement** au job
`analyze` qui en a besoin (moindre privilège). La fonctionnalité CodeQL est
inchangée : le job conserve `contents: read` et `security-events: write`.

### Fuzzing (0 → hausse attendue)

Ajout d'une baseline `cargo-fuzz` (moteur libFuzzer) via le workflow
[`fuzz.yml`](../.github/workflows/fuzz.yml) et le crate
[`fuzz/`](../fuzz). Trois cibles fuzzent des fonctions **pures** réellement
sensibles, sans base de données, réseau ni shell :

- `mdfmt_escape` — échappement Markdown (`src/mdfmt.rs`) ;
- `secret_detection` — détection/redaction de secrets (`src/secrets.rs`) ;
- `date_filter_parse` — parsing des durées/dates des filtres (`src/db.rs`,
  `src/prune.rs`).

Rust nightly et `cargo-fuzz` (version épinglée) ne sont utilisés **que** dans ce
workflow ; le build, les tests et les releases restent sur la toolchain stable.
Détails dans [docs/FUZZING.md](FUZZING.md).

## CII-Best-Practices and Contributors

Le badge **CII-Best-Practices** (niveau Passing) est désormais **obtenu** ; le
check `Contributors` reste à **0**. Les deux sont traités honnêtement, sans score
gaming.

### CII-Best-Practices (badge Passing obtenu)

- **État** : le badge OpenSSF Best Practices **niveau Passing** est accordé —
  projet <https://www.bestpractices.dev/projects/13366>. Le badge est affiché
  dans le README.
- **Preuves** : le dossier
  [docs/OPENSSF_BEST_PRACTICES.md](OPENSSF_BEST_PRACTICES.md) recense les éléments
  justifiant chaque réponse (licence, politique de sécurité, CI, SAST, fuzzing,
  audit des dépendances, intégrité des releases, etc.).
- **Limite** : le check Scorecard lit le badge de façon asynchrone ; il passera de
  **0** à **10** au prochain scan, sans action supplémentaire.

### Contributors (0)

- **État** : Scorecard ne détecte aucune organisation contributrice (le score
  reflète des contributions issues de plusieurs entreprises ou organisations).
- **Action saine** : améliorer l'accueil contributeur — ajout de
  [CONTRIBUTING.md](../CONTRIBUTING.md), d'un [code de conduite](../CODE_OF_CONDUCT.md)
  (Contributor Covenant v2.1), de modèles d'issues
  ([.github/ISSUE_TEMPLATE/](../.github/ISSUE_TEMPLATE)) et d'un modèle de PR
  ([.github/pull_request_template.md](../.github/pull_request_template.md)) — afin
  de faciliter de **vraies** contributions externes à l'avenir.
- **Action refusée** : ne **jamais** créer de faux contributeurs ni de fausse
  organisation pour gonfler ce score. Il ne progressera légitimement qu'avec de
  réelles contributions multi-organisations, qui prennent du temps.

## Points reportés (PR dédiées ou actions hors code)

- **Vulnerabilities** : `lru` et `paste` proviennent de `ratatui 0.29.0` (version
  stable courante, qui dépend encore de `lru 0.12`). Les avis sont des
  `unmaintained`/`unsound` sans correctif simple ; ils sont autorisés dans
  [deny.toml](../deny.toml) et seront résorbés lors d'une mise à jour future de
  Ratatui.
- **Branch-Protection / Code-Review** : exigent un durcissement des rulesets
  (approbation obligatoire, revue par code owner, status checks requis). Ces
  réglages ne sont pas modifiés sans demande explicite. Un fichier `CODEOWNERS`
  n'apporte de gain que si le ruleset impose la revue par code owner ; il pourra
  être ajouté conjointement à ce durcissement.

## Limites assumées

- `Maintained`, `Contributors` et `Packaging` dépendent de facteurs temporels,
  organisationnels ou de mode de distribution propres au projet : ils ne sont pas
  « jouables » par des changements cosmétiques et ne sont pas forcés artificiellement.
