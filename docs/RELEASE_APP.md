# GitHub App de release (`vesperis-mnemo-release`)

Le workflow [`.github/workflows/release.yml`](../.github/workflows/release.yml)
publie chaque release de `mnemo` (commit de bump, tag `vX.Y.Z`, GitHub Release
et artefacts signés). Pour pousser ce commit et ce tag sur des références
**protégées par des rulesets**, il s'authentifie via une **GitHub App dédiée**
plutôt qu'avec un *Personal Access Token* (PAT) long terme.

> **Pourquoi une GitHub App et pas un PAT ?**
> Le jeton d'une GitHub App est **généré à l'exécution du workflow**, a une
> **durée de vie courte** (≈ 1 h), est **limité au seul dépôt** `mnemo` et aux
> **permissions minimales** de l'App. Aucun secret long terme à faire tourner,
> surface d'attaque réduite, et révocation immédiate en désinstallant l'App.

---

## 1. Vue d'ensemble

| Élément | Valeur |
| --- | --- |
| Nom de l'App | `vesperis-mnemo-release` |
| Propriétaire | organisation `Vesperis-group` |
| Installation | **uniquement** sur le dépôt `mnemo` |
| Action consommatrice | `actions/create-github-app-token` (épinglée par SHA) |
| Variable Actions | `MNEMO_RELEASE_APP_CLIENT_ID` |
| Secret Actions | `MNEMO_RELEASE_APP_PRIVATE_KEY` |

Le workflow lit ces deux entrées :

```yaml
- name: Generate GitHub App token
  id: app-token
  uses: actions/create-github-app-token@<sha> # v3.2.0
  with:
    client-id: ${{ vars.MNEMO_RELEASE_APP_CLIENT_ID }}
    private-key: ${{ secrets.MNEMO_RELEASE_APP_PRIVATE_KEY }}
    owner: ${{ github.repository_owner }}
    repositories: mnemo
```

Le `token` généré (`steps.app-token.outputs.token`) est ensuite utilisé par
`actions/checkout` (push du commit/tag) et par `release-it` (création de la
GitHub Release).

---

## 2. Permissions de l'App (principe du moindre privilège)

Configurer **exactement** ces permissions, et rien de plus :

**Repository permissions**

| Permission | Niveau | Pourquoi |
| --- | --- | --- |
| Contents | **Read and write** | pousser le commit de release + le tag, créer la Release |
| Pull requests | **Read-only** | lecture du contexte des PR (changelog) |
| Metadata | Read-only (automatique) | requis par GitHub pour toute App |

**À NE PAS accorder** : Administration, Actions (write), Issues (write),
Secrets (write), Workflows, Packages, ni aucune permission d'organisation.

L'App n'a **pas** besoin de s'abonner à des *events* webhook : décocher
« Active » dans la section Webhook.

---

## 3. Création de l'App (étapes manuelles GitHub)

> Ces étapes sont **manuelles** et réalisées par un propriétaire de
> l'organisation `Vesperis-group`. Elles ne sont pas automatisables depuis ce
> dépôt.

1. **Organisation → Settings → Developer settings → GitHub Apps → New GitHub App.**
2. **GitHub App name** : `vesperis-mnemo-release`.
3. **Homepage URL** : l'URL du dépôt (`https://github.com/Vesperis-group/mnemo`).
4. **Webhook** : décocher **Active** (aucun webhook nécessaire).
5. **Repository permissions** : appliquer le tableau de la section 2
   (Contents: Read and write, Pull requests: Read-only).
6. **Where can this GitHub App be installed?** : **Only on this account**.
7. Cliquer **Create GitHub App**.

### 3.1 Récupérer le Client ID

Sur la page de l'App, noter le **Client ID** (une chaîne de la forme
`Iv23li...`, affichée dans la section **About** en haut de la page de l'App, à
côté de l'App ID). Il alimentera la variable `MNEMO_RELEASE_APP_CLIENT_ID`.

> ⚠️ Le **Client ID** est **différent** de l'**App ID** : l'App ID est un
> entier, le Client ID est une chaîne préfixée `Iv23li`. L'action
> `create-github-app-token` attend désormais le **Client ID** via l'entrée
> `client-id` (l'entrée `app-id` est dépréciée).

### 3.2 Générer la clé privée

Sur la page de l'App → **Private keys → Generate a private key**. Un fichier
`*.pem` est téléchargé. Son contenu **intégral** (lignes
`-----BEGIN/END ...-----` comprises) alimentera le secret
`MNEMO_RELEASE_APP_PRIVATE_KEY`.

> ⚠️ La clé privée n'est téléchargeable qu'**une seule fois**. Conservez-la dans
> un gestionnaire de secrets, ne la committez **jamais**, et révoquez-la
> (regénérez-en une) au moindre doute.

### 3.3 Installer l'App sur le dépôt

Page de l'App → **Install App** → choisir l'organisation `Vesperis-group` →
**Only select repositories** → sélectionner **uniquement** `mnemo` → **Install**.

---

## 4. Configurer la variable et le secret dans le dépôt

Dans **`mnemo` → Settings → Secrets and variables → Actions** :

- onglet **Variables** → **New repository variable** :
  - **Name** : `MNEMO_RELEASE_APP_CLIENT_ID`
  - **Value** : le Client ID (chaîne `Iv23li...`) de la section 3.1.
- onglet **Secrets** → **New repository secret** :
  - **Name** : `MNEMO_RELEASE_APP_PRIVATE_KEY`
  - **Value** : le contenu complet du fichier `.pem` (section 3.2).

> Le Client ID n'est pas sensible (il peut être une *variable*). Seule la clé
> privée est un *secret*. Aucune de ces valeurs n'est jamais affichée dans les
> logs : `create-github-app-token` masque automatiquement le jeton généré.

---

## 5. Ajouter l'App aux *bypass lists* des rulesets

Les rulesets protègent `main` et les tags `v*` (voir
[`REPOSITORY_RULES.md`](REPOSITORY_RULES.md)). Pour que le workflow puisse
pousser le commit de release et le tag, l'App doit être autorisée à **contourner**
ces protections, et **elle seule** :

- ruleset **« Protect main »** → **Bypass list** → **Add bypass** →
  sélectionner l'App `vesperis-mnemo-release`.
- ruleset **« Protect release tags »** → **Bypass list** → **Add bypass** →
  sélectionner l'App `vesperis-mnemo-release`.

Aucun utilisateur humain ni le `GITHUB_TOKEN` par défaut ne doit figurer dans
ces listes.

---

## 6. Vérification

1. Ouvrir une PR de test contenant un *Conventional Commit* (ex. `fix: ...`),
   la faire passer la CI, puis la merger dans `main`.
2. Le workflow **Release** se déclenche : l'étape *Generate GitHub App token*
   doit réussir, puis le commit `chore: release vX.Y.Z [skip ci]` et le tag
   `vX.Y.Z` doivent être poussés sans erreur de protection de branche.
3. Une GitHub Release `mnemo vX.Y.Z` apparaît avec ses artefacts signés.

Si le push échoue avec une erreur de *ruleset*/protection, vérifier que l'App
figure bien dans les deux bypass lists (section 5).

---

## 7. Rotation et révocation

- **Rotation de la clé** : générer une nouvelle clé privée (section 3.2),
  mettre à jour le secret `MNEMO_RELEASE_APP_PRIVATE_KEY`, puis supprimer
  l'ancienne clé côté App.
- **Révocation d'urgence** : désinstaller l'App du dépôt (section 3.3) coupe
  immédiatement toute capacité de publication ; aucun jeton long terme ne reste
  valide puisque le token est éphémère.

---

## 8. Rappels de sécurité

- **Aucun PAT long terme** n'est utilisé pour la release.
- Le jeton de l'App est **généré à l'exécution** et expire rapidement.
- La clé privée n'est **jamais** committée ni journalisée.
- Les permissions de l'App restent **minimales** (Contents: write,
  Pull requests: read).
- L'App est installée **uniquement** sur `mnemo`.
