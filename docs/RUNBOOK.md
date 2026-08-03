# Runbook

`mnemo runbook` génère un document **Markdown ou JSON réutilisable** à partir
des commandes d'une session de travail ou d'un projet Git. Le résultat est
déterministe (mêmes entrées → même sortie) et directement exploitable comme
procédure de release, rapport d'audit, post-mortem d'incident ou export pour
un outil ITSM.

Caractéristiques principales :

- Tri chronologique strict (la commande la plus ancienne en premier).
- Commandes vides exclues automatiquement.
- **Secrets redactés par défaut** — chaque commande est analysée avant
  inclusion ; `--no-redact` désactive ce comportement.
- Sortie sur stdout ou dans un fichier (`--output`) ; jamais d'écrasement
  silencieux sans `--force`.

---

## Commandes

### Sources (mutuellement exclusives)

| Option | Description |
| --- | --- |
| `--last` | Cible la dernière session enregistrée. |
| `--session <ID>` | Session explicite identifiée par son `session_id`. |
| `--project <NOM\|CHEMIN>` | Toutes les commandes d'un projet (nom court ou chemin `git_root`). |

Exactement un de ces trois drapeaux est requis. Une erreur claire est affichée
si aucun n'est fourni ou si plusieurs sont combinés.

### Options de sortie

| Option | Défaut | Description |
| --- | --- | --- |
| `--output <FICHIER>` | stdout | Fichier de destination. |
| `--force` | non | Autorise l'écrasement d'un fichier existant. |
| `--format markdown\|json` | `markdown` | Format de sortie. |

### Options de contenu

| Option | Défaut | Description |
| --- | --- | --- |
| `--limit <N>` | aucune | Nombre maximal de commandes incluses. |
| `--title <TITRE>` | identifiant de session ou nom de projet | Titre du document. |
| `--no-redact` | désactivé | Désactive la redaction des secrets. |
| `--group-by none\|cwd\|project` | `none` | Mode de groupement des commandes. |

### Exemples rapides

```bash
# Runbook de la dernière session sur stdout
mnemo runbook --last

# Runbook d'une session précise avec titre
mnemo runbook --session abc123 --title "Session debug 2026-08-03"

# Runbook d'un projet complet, limité aux 50 dernières commandes
mnemo runbook --project mnemo --limit 50

# Écriture dans un fichier (refusé si le fichier existe déjà)
mnemo runbook --last --output work-session.md

# Écrasement explicite
mnemo runbook --last --output work-session.md --force

# Export JSON pipeable
mnemo runbook --last --format json | jq '.commands | length'

# Groupement par répertoire
mnemo runbook --project mnemo --group-by cwd

# Désactiver la redaction pour un audit interne
mnemo runbook --last --no-redact
```

---

## Cas d'usage DevSecOps

### Générer un runbook de release

Documente les commandes effectuées sur le projet lors d'un cycle de release.
Le fichier produit peut être archivé dans le dépôt ou joint à la GitHub Release.

```bash
mnemo runbook --project ~/mnemo \
  --title "Release v1.6.23" \
  --output docs/runbooks/release-v1.6.23.md
```

Exemple de résultat (`docs/runbooks/release-v1.6.23.md`) :

```text
# Runbook - Release v1.6.23

## Metadata

- Source: projet mnemo
- Generated at: 2026-08-03 11:00:00
- Commands: 4

## Commands

### 1. ~/mnemo

    cargo fmt --all -- --check

### 2. ~/mnemo

    cargo test --locked

### 3. ~/mnemo

    git tag v1.6.23 -m "Release v1.6.23"

### 4. ~/mnemo

    git push origin v1.6.23
```

### Générer un runbook d'audit supply chain

Capture toutes les commandes effectuées sur le projet pendant une période
d'audit, regroupées par répertoire pour faciliter la relecture.

```bash
mnemo runbook --project ~/mnemo \
  --title "Audit supply chain 2026-08" \
  --group-by cwd \
  --output docs/runbooks/audit-2026-08.md
```

Le groupement `--group-by cwd` produit des sections de niveau `##` par
répertoire de travail, puis des sous-sections numérotées par commande.

### Générer un runbook d'incident post-mortem

Documente les actions d'investigation et de remédiation d'un incident.
`--no-redact` est utilisé ici pour un audit interne où les valeurs complètes
sont nécessaires — à réserver aux contextes de confiance.

```bash
mnemo runbook --last \
  --title "Incident 2026-08-03 — restauration base" \
  --no-redact \
  --output docs/runbooks/incident-2026-08-03.md
```

> ⚠️ Avec `--no-redact`, les commandes contenant des secrets apparaissent en
> clair dans le document produit. N'utilisez ce mode que dans des contextes de
> confiance et ne committez pas le fichier résultant dans un dépôt public.

### Générer un export JSON pour un ticket ITSM

Produit un JSON structuré, stable et déterministe, directement parseable par
`jq` ou un outil ITSM.

```bash
mnemo runbook --session abc123 --format json | jq .
```

Exemple de sortie JSON :

```json
{
  "title": "abc123",
  "source": "session abc123",
  "generated_at": "2026-08-03 11:05:00",
  "commands": [
    {
      "n": 1,
      "cwd": "~/mnemo",
      "timestamp": "2026-08-03 10:50:00",
      "command": "cargo clippy --locked --all-targets --all-features -- -D warnings"
    },
    {
      "n": 2,
      "cwd": "~/mnemo",
      "timestamp": "2026-08-03 10:52:00",
      "command": "cargo test --locked"
    }
  ]
}
```

Avec groupement (`--group-by cwd` ou `--group-by project`), chaque objet
commande porte un champ `"group"` supplémentaire.

---

## Sécurité

### Redaction activée par défaut

Sans `--no-redact`, chaque commande est analysée par le moteur de détection de
secrets de mnemo avant d'être incluse dans le runbook. Les valeurs sensibles
détectées sont remplacées par `[REDACTED]`  ; en cas d'ambiguïté sur l'ensemble
de la commande, elle devient `[REDACTED COMMAND]`.

La redaction s'applique à :

- Jetons porteurs (`Bearer …`, `Authorization: …`).
- Affectations de variables sensibles (`TOKEN=…`, `API_KEY=…`, `PASSWORD=…`).
- URLs avec identifiants (`user:motdepasse@hôte`).
- Options explicites (`--password`, `--token`, `--secret`, `--api-key`, etc.).
- Fragments de clé privée (`-----BEGIN …`).
- Mots de passe de clients SQL (MySQL, psql, etc.).

La liste des mots-clés déclencheurs est configurable dans
`~/.config/mnemo/config.toml` (section `sensitive_keywords`). Pour les détails
de l'algorithme, voir [docs/SECRETS.md](SECRETS.md).

### Comportement de `--no-redact`

`--no-redact` désactive entièrement la redaction : les commandes sont incluses
telles qu'elles sont stockées en base. Utilisez ce mode uniquement dans des
contextes de confiance (terminal local, audit interne isolé). Ne committez pas
un runbook généré avec `--no-redact` dans un dépôt partagé ou public si les
sessions concernées ont pu contenir des secrets.

> ℹ️ La redaction est heuristique, pas exhaustive. Vérifiez manuellement les
> runbooks produits avant de les partager si les sessions concernées ont
> manipulé des informations sensibles.

---

## Format Markdown

Le format Markdown produit (défaut, `--format markdown`) est composé de trois
sections principales.

### Structure du document

```text
# Runbook - <TITRE>

## Metadata

- Source: <description de la source>
- Generated at: <YYYY-MM-DD HH:MM:SS>
- Commands: <nombre de commandes>

## Commands

### 1. <cwd>

```bash
<commande>
```

### 2. <cwd>

```bash
<commande>
```
```

Avec `--group-by cwd` ou `--group-by project` :

```text
## <clé de groupe>

### 1.

```bash
<commande>
```

### 2.

```bash
<commande>
```
```

### Propriétés

- Le titre du document reprend `--title` ou, par défaut, l'identifiant de
  session ou le nom du projet.
- `Generated at` est l'horodatage UTC au moment de l'exécution.
- Les backticks dans les commandes sont échappés automatiquement (les blocs de
  code utilisent des délimiteurs plus longs que le contenu).
- En l'absence de commandes, la section `## Commands` contient `_Aucune
  commande._` plutôt que rien.

---

## Format JSON

Le format JSON (`--format json`) produit un document compact, stable et
déterministe, compatible avec `jq` et les pipelines CI.

### Structure

```json
{
  "title": "<TITRE>",
  "source": "<description de la source>",
  "generated_at": "<YYYY-MM-DD HH:MM:SS>",
  "commands": [
    {
      "n": 1,
      "cwd": "<répertoire de travail>",
      "timestamp": "<YYYY-MM-DD HH:MM:SS>",
      "command": "<commande>"
    }
  ]
}
```

Avec `--group-by cwd` ou `--group-by project`, chaque objet commande porte
un champ `"group"` supplémentaire :

```json
{
  "n": 1,
  "cwd": "~/mnemo",
  "timestamp": "2026-08-03 10:50:00",
  "command": "cargo test --locked",
  "group": "~/mnemo"
}
```

Le champ `"group"` est omis (`skip_serializing_if`) en mode `--group-by none`.

### Stabilité

Le JSON produit est **déterministe** : mêmes entrées et même horodatage →
même sortie byte-for-byte. L'ordre des commandes est chronologique croissant
(le plus ancien en premier) ; avec groupement, l'ordre des groupes est
alphabétique et les commandes dans chaque groupe conservent l'ordre
chronologique.

---

## Groupement (`--group-by`)

| Mode | Comportement |
| --- | --- |
| `none` (défaut) | Liste plate : une section numérotée `### N. <cwd>` par commande. |
| `cwd` | Sections `## <répertoire>` ; commandes numérotées dans chaque section. |
| `project` | Sections `## <racine Git>` ; commandes numérotées dans chaque section. |

Pour `--group-by project`, les commandes sans contexte Git sont regroupées
sous la clé `(sans projet)`.

### Exemple `--group-by cwd`

```bash
mnemo runbook --project mnemo --group-by cwd --output audit.md
```

```text
## ~/mnemo

### 1.

    cargo fmt --all -- --check

### 2.

    cargo test --locked

## ~/mnemo/fuzz

### 1.

    cargo fuzz run mdfmt_escape -- -max_total_time=30
```

### Exemple `--group-by project`

Utile lorsqu'un projet agrège des commandes issues de plusieurs racines Git
(monorepo, sous-modules, etc.).

---

## Limites connues

- **Commandes sans session** : les commandes importées depuis `~/.bash_history`
  avant l'installation du hook mnemo ne sont pas rattachées à une session
  (`MNEMO_SESSION_ID` absent). Elles n'apparaissent pas avec `--last` ou
  `--session`, mais sont accessibles via `--project`.

- **Filtres temporels** : `mnemo runbook` n'expose pas encore de filtre
  `--since`/`--until`. Pour borner la plage temporelle, combinez `--limit N`
  (qui retient les N commandes les plus récentes avant tri) ou exportez d'abord
  avec `mnemo export --format json` puis filtrez avec `jq`.

- **Redaction heuristique** : la détection est raisonnable mais non exhaustive.
  Relisez toujours un runbook avant de le partager si la session concernée a
  manipulé des secrets inhabituels.

- **Format ANSI** : les séquences de couleur ANSI présentes dans les commandes
  enregistrées apparaissent telles quelles dans le document produit. Utilisez
  `--no-redact` avec précaution si des sorties colorées ont été copiées.
