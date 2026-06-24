# OpenSSF Scorecard

Cette page documente la posture [OpenSSF Scorecard](https://scorecard.dev) du
dépôt : score courant, checks faibles, corrections appliquées et limites
assumées. Le workflow d'évaluation est décrit dans [docs/CI_CD.md](CI_CD.md)
(`scorecard.yml`).

## Score courant

- **Score global** : 5,8 / 10 (relevé du 2026-06-24).
- Source : `https://api.scorecard.dev/projects/github.com/Vesperis-group/mnemo`.

## Diagnostic par check

| Check | Score | Raison | Action | Priorité |
| --- | ---: | --- | --- | --- |
| CI-Tests | 10 | Tests exécutés sur les PR | — | — |
| SAST | 10 | CodeQL actif | — | — |
| Pinned-Dependencies | 10 | Actions épinglées par SHA | — | — |
| Dangerous-Workflow | 10 | Aucun motif dangereux | — | — |
| Binary-Artifacts | 10 | Aucun binaire commité | — | — |
| Security-Policy | 10 | `SECURITY.md` présent | — | — |
| License | 10 | Licence MIT détectée | — | — |
| Token-Permissions | 9 → 10 | `security-events: write` au niveau workflow dans `codeql.yml` | Déplacé au seul job `analyze` | A (fait) |
| Signed-Releases | 8 | Provenance sur le fichier de checksums, pas sur chaque artefact | Logique release (non modifiée ici) | C |
| Vulnerabilities | 8 | `RUSTSEC-2026-0002` (lru) et `RUSTSEC-2024-0436` (paste), transitifs via `ratatui` | Suivi, PR dédiée | B |
| Branch-Protection | 3 | Protection non maximale sur `main` (pas d'approbation requise, pas de status checks) | Rulesets (non modifiés ici) | C |
| Code-Review | 0 | 0/30 changesets approuvés | Nécessite revue + approbation (rulesets) | C |
| Dependency-Update-Tool | 0 → 10 | Aucun outil de mise à jour détecté | `.github/dependabot.yml` ajouté | A (fait) |
| Fuzzing | 0 | Projet non fuzzé | PR dédiée `feat/ci-fuzzing-baseline` | B (reporté) |
| CII-Best-Practices | 0 | Pas de badge OpenSSF Best Practices | Inscription manuelle du mainteneur | B (reporté) |
| Maintained | 0 | Projet créé il y a moins de 90 jours | S'améliore avec le temps | — |
| Contributors | 0 | 0 organisation contributrice | Hors de portée d'une PR | — |
| Packaging | -1 | Aucun workflow de packaging reconnu | Non concluant (publication binaire via GitHub App) | — |

## Corrections appliquées dans cette PR

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

## Points reportés (PR dédiées ou actions hors code)

- **Fuzzing** : intégrer une baseline `cargo-fuzz` est un chantier à part entière
  (`feat/ci-fuzzing-baseline`). Aucun fuzzing incomplet n'est ajouté ici pour ne
  pas fausser le score.
- **Vulnerabilities** : `lru` et `paste` proviennent de `ratatui 0.29.0` (version
  stable courante, qui dépend encore de `lru 0.12`). Les avis sont des
  `unmaintained`/`unsound` sans correctif simple ; ils sont autorisés dans
  [deny.toml](../deny.toml) et seront résorbés lors d'une mise à jour future de
  Ratatui.
- **CII-Best-Practices** : nécessite l'inscription du projet sur
  <https://www.bestpractices.dev> par un mainteneur (action manuelle, hors dépôt).
- **Branch-Protection / Code-Review** : exigent un durcissement des rulesets
  (approbation obligatoire, revue par code owner, status checks requis). Ces
  réglages ne sont pas modifiés sans demande explicite. Un fichier `CODEOWNERS`
  n'apporte de gain que si le ruleset impose la revue par code owner ; il pourra
  être ajouté conjointement à ce durcissement.

## Limites assumées

- `Maintained`, `Contributors` et `Packaging` dépendent de facteurs temporels,
  organisationnels ou de mode de distribution propres au projet : ils ne sont pas
  « jouables » par des changements cosmétiques et ne sont pas forcés artificiellement.
