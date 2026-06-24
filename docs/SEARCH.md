# Recherche et récupération de commandes

mnemo aide à **retrouver, inspecter et récupérer** une commande de l'historique,
sans jamais l'exécuter automatiquement. Cette page détaille `mnemo search`,
`mnemo show` et `mnemo print`.

> Principe de sécurité : mnemo n'exécute **jamais** une commande issue de
> l'historique. `show` et `print` lisent la base et affichent du texte. Si vous
> copiez ou relancez une commande, vous en restez responsable.

## `mnemo search` : retrouver des commandes

Sans `--print`, `mnemo search` ouvre la TUI interactive avec les filtres
pré-remplis. Avec `--print` (ou `--json` / `--id-only`), la sortie est non
interactive, adaptée aux scripts et à la CI.

### Filtres combinables

Tous les filtres se combinent par ET logique :

```bash
mnemo search docker --print --failed           # uniquement les échecs
mnemo search --print --exit-code 127           # code de sortie exact
mnemo search cargo --print --project mnemo      # par projet (nom ou git_root)
mnemo search --print --branch main             # par branche Git
mnemo search --print --cwd /home/killian/mnemo # par répertoire de travail exact
mnemo search --print --shell bash              # par shell
mnemo search --print --limit 50                # borne le nombre de résultats
```

### Bornes temporelles

`--since` accepte une **durée** ou une **date** ; `--before` (alias `--until`)
attend une date.

```bash
mnemo search --print --since 24h               # dernières 24 heures
mnemo search --print --since 7d                # 7 derniers jours
mnemo search --print --since 2026-01-01        # depuis une date (AAAA-MM-JJ)
mnemo search --print --before 2026-06-01       # avant une date
mnemo search --print --until 2026-06-01        # identique à --before
```

Unités de durée acceptées : `h` (heures), `d` (jours), `w` (semaines, 7 jours),
`m` (mois, 30 jours), `y` (années, 365 jours). Une valeur de date **invalide**
n'interrompt pas la commande : le filtre est ignoré, avec un avertissement sur
stderr.

### Formats de sortie

```bash
mnemo search cargo --json                      # JSON stable (mode non interactif implicite)
mnemo search cargo --id-only                   # uniquement les IDs, un par ligne
```

- `--json` produit un tableau d'objets au format stable, identique à
  `mnemo export --format json`.
- `--id-only` n'affiche que les identifiants, un par ligne, pratique à chaîner.
- `--json` et `--id-only` activent automatiquement le mode non interactif (pas
  besoin de `--print`) et sont mutuellement exclusifs.

## `mnemo show <id>` : inspecter une commande

Affiche le détail complet d'une commande identifiée par son ID.

```text
$ mnemo show 128
Commande 128

Date : 2026-06-23 14:22:10
Dossier : /home/killian/projects/mnemo
Projet Git : /home/killian/projects/mnemo
Branche : main
Shell : bash
Session : 20260623T141200-12345
Code retour : 0

Commande :
cargo test --locked
```

- Les champs non renseignés sont **omis** (jamais inventés).
- Une commande déjà nettoyée par `mnemo secrets redact` apparaît sous sa forme
  **redactée** stockée.
- Un ID inexistant produit une erreur claire et un code de sortie non nul.

## `mnemo print <id>` : récupérer la commande brute

Imprime uniquement la commande sur stdout, sans label ni couleur, suivie d'un
saut de ligne. Utile pour copier ou rediriger.

```bash
$ mnemo print 128
cargo test --locked
```

mnemo **n'exécute pas** la commande : `print` sert à la récupérer, pas à la
lancer. Pour la réexécuter, c'est à vous de le faire explicitement, par exemple :

```bash
# Relecture avant exécution manuelle (jamais automatique)
mnemo show 128
# Puis, en connaissance de cause :
eval "$(mnemo print 128)"   # à vos risques, mnemo ne fait pas cela pour vous
```

Un ID inexistant produit une erreur sur stderr et un code de sortie non nul.

## Comment obtenir un ID

```bash
mnemo list                    # 20 dernières commandes avec leurs IDs
mnemo search cargo --id-only  # IDs des correspondances, un par ligne
```

Dans la TUI, l'ID de la commande sélectionnée est également visible.

## Récapitulatif

| Commande | Rôle | Exécute la commande ? |
| --- | --- | --- |
| `mnemo search` | Retrouver et filtrer | Non |
| `mnemo show <id>` | Inspecter en détail | Non |
| `mnemo print <id>` | Récupérer le texte brut | Non |
