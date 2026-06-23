# Règles de dépôt recommandées (Rulesets)

Ce document décrit la configuration de **protection** recommandée pour le dépôt
`Vesperis-group/mnemo`. Ces réglages sont **manuels** côté GitHub
(**Settings → Rules → Rulesets**) : ils ne sont pas gérés par l'API depuis ce
dépôt. Le workflow [`branch-policy.yml`](../.github/workflows/branch-policy.yml)
n'est qu'un garde-fou *best-effort* et **ne remplace pas** ces rulesets.

Deux rulesets sont recommandés :

- **Protect main** — protège la branche par défaut `main`.
- **Protect release tags** — protège les tags de version `v*`.

---

## 1. Ruleset « Protect main »

- **Target** : branche `main` (Include default branch).
- **Bypass list** : **uniquement** l'App
  [`vesperis-mnemo-release`](RELEASE_APP.md) (pour pousser le commit de release).
  Aucun humain, pas le `GITHUB_TOKEN` par défaut.
- **Rules** recommandées :
  - **Require a pull request before merging** (au moins 1 review ;
    *Dismiss stale approvals* ; *Require conversation resolution*).
  - **Require status checks to pass** (voir la liste ci-dessous), avec
    **Require branches to be up to date**.
  - **Block force pushes**.
  - **Restrict deletions**.
  - *(optionnel)* **Require signed commits** — compatible avec l'App de release,
    dont les commits poussés via le jeton sont vérifiés.

### Required status checks

À cocher comme **obligatoires** (les noms correspondent aux *jobs* des
workflows) :

| Check (job) | Workflow |
| --- | --- |
| `Rust (fmt, clippy, test, build)` | `ci.yml` |
| `Shell scripts (bash -n)` | `ci.yml` |
| `actionlint (workflows)` | `lint.yml` |
| `ShellCheck (scripts)` | `lint.yml` |
| `cargo audit / deny / machete` | `audit.yml` |
| `gitleaks (détection de secrets)` | `audit.yml` |
| `CodeQL (Rust)` | `codeql.yml` |

> ⚠️ **Ne pas** rendre le workflow **Release** obligatoire : il s'exécute
> **après** le merge sur `main` (sur l'évènement `push`), il ne peut donc pas
> être un check bloquant de PR.
>
> Astuce : un check n'apparaît dans la liste sélectionnable qu'après s'être
> exécuté **au moins une fois** sur le dépôt. Ouvrez une première PR pour que
> `actionlint`, `ShellCheck` et `CodeQL` deviennent sélectionnables.

---

## 2. Ruleset « Protect release tags »

- **Target** : tags correspondant à `v*` (Ref name → `refs/tags/v*`).
- **Bypass list** : **uniquement** l'App `vesperis-mnemo-release` (pour créer le
  tag `vX.Y.Z` pendant la release).
- **Rules** recommandées :
  - **Restrict creations** (seule l'App, via la bypass list, crée des tags `v*`).
  - **Restrict updates** et **Restrict deletions** (les tags de version sont
    immuables une fois publiés).
  - **Block force pushes**.

---

## 3. Pourquoi l'App doit être en bypass

Le job `publish` de `release.yml` pousse, sur `main`, le commit
`chore: release vX.Y.Z [skip ci]` puis le tag `vX.Y.Z`. Sans bypass, les
rulesets ci-dessus **rejetteraient** ce push (PR requise, création de tag
restreinte). En ajoutant **uniquement** l'App `vesperis-mnemo-release` aux deux
bypass lists, on autorise précisément l'automatisation de release **sans**
affaiblir la protection pour les humains. Détails et création de l'App :
[`RELEASE_APP.md`](RELEASE_APP.md).

---

## 4. Tester la configuration

1. Créer une branche dédiée, ouvrir une **PR de test** vers `main`.
2. Vérifier que **tous** les checks requis apparaissent et bloquent le merge
   tant qu'ils échouent.
3. Vérifier qu'un **push direct** sur `main` (hors App) est refusé.
4. Merger la PR et confirmer que le workflow **Release** pousse commit + tag
   sans erreur de protection (App en bypass fonctionnelle).
5. Confirmer qu'aucun humain ne peut supprimer/forcer `main` ni un tag `v*`.
