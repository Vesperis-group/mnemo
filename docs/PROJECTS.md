# Activité par projet

`mnemo project` regroupe l'historique déjà stocké par **projet** afin de
naviguer par dépôt et de produire des **rapports d'activité** réutilisables
(Markdown ou JSON). C'est utile pour faire le point sur un dépôt, documenter une
intervention, préparer une revue ou archiver ce qui a été fait sur une période.

Comme le reste de mnemo, ces commandes sont en **lecture seule** : elles
n'exécutent jamais les commandes de l'historique, ne modifient pas la base et ne
changent pas le schéma. Les commandes déjà redactées (`mnemo secrets`) sont
restituées telles quelles, sans nouvelle analyse.

## Qu'est-ce qu'un projet ?

Un projet est l'ensemble des commandes partageant une même **racine Git**
(`git_root`), capturée au moment de l'enregistrement. Le nom court du projet est
le dernier segment de cette racine (par exemple `mnemo` pour
`/home/u/dev/mnemo`).

Les commandes enregistrées **hors d'un dépôt Git** n'ont pas de `git_root` et ne
sont rattachées à aucun projet : elles restent consultables via `mnemo search`,
mais ne sont pas regroupées ici. mnemo ne fabrique jamais de projet artificiel.

## Lister les projets connus

```bash
mnemo project list                 # tableau lisible
mnemo project list --limit 10      # bornage du nombre de projets
mnemo project list --json          # sortie JSON stable
```

Chaque projet est résumé par son nombre de commandes, son nombre de sessions
distinctes, sa dernière activité et les branches Git rencontrées. Les projets
sont triés du plus récemment actif au plus ancien.

## Détailler un projet

```bash
mnemo project show mnemo           # par nom court
mnemo project show /home/u/dev/mnemo   # par racine complète
mnemo project show --current       # projet du répertoire courant
mnemo project show mnemo --json    # sortie JSON
mnemo project show mnemo --limit 50    # plus de commandes récentes
```

`project show` affiche :

- les métadonnées du projet : racine, remote, nombre de commandes et de
  sessions, période d'activité (première et dernière commande), branches ;
- les **commandes récentes** ;
- les **derniers échecs** (code de sortie non nul), le cas échéant.

### Résolution du projet

L'argument accepte le **nom court** (suffixe de la racine) ou la **racine
complète**. Un nom préfixé par `~` est étendu vers le répertoire personnel. Si
plusieurs racines correspondent au même nom court, mnemo refuse l'ambiguïté et
demande la racine complète. `--current` détecte le projet du répertoire courant
(racine Git en priorité) et échoue clairement s'il est absent de l'historique.

## Générer un rapport d'activité

```bash
mnemo project report --current                       # Markdown sur stdout
mnemo project report mnemo --since 30d               # 30 derniers jours
mnemo project report mnemo --since 2026-01-01 --until 2026-07-01
mnemo project report mnemo --format json             # JSON structuré
mnemo project report mnemo --output rapport.md       # écriture dans un fichier
mnemo project report mnemo --output rapport.md --force   # écrasement autorisé
mnemo project report mnemo --limit 200               # borne le détail chronologique
```

Le rapport contient les **agrégats de la période** (commandes, échecs, sessions,
première et dernière activité, branches), un **bloc de commandes** réutilisable,
un **détail chronologique** sous forme de tableau, et la liste des **échecs**.

### Période

- `--since` accepte une durée (`24h`, `7d`, `2w`, `3m`, `1y`) ou une date
  `AAAA-MM-JJ` (incluse).
- `--until` accepte une date `AAAA-MM-JJ` (exclue) ou une durée.
- Une valeur invalide est refusée explicitement, pour ne jamais produire un
  rapport silencieusement erroné.

### Sortie fichier

Sans `--output`, le rapport est écrit sur la sortie standard. Avec `--output`, un
fichier existant n'est **jamais écrasé sans `--force`**.

## Robustesse du Markdown

Le rendu Markdown échappe les caractères susceptibles de casser un document : les
pipes (`|`) des cellules de tableau sont protégés, les retours à la ligne sont
neutralisés, et les blocs de code choisissent une clôture plus longue que toute
suite de backticks interne. Une commande contenant `|`, des backticks ou des
retours à la ligne ne déforme donc jamais le rapport.

## Garanties

- **Lecture seule** : aucune commande de l'historique n'est exécutée, la base
  n'est pas modifiée, le schéma est inchangé.
- **Pas de fuite** : les commandes redactées restent redactées ; aucune nouvelle
  analyse de secrets n'est déclenchée.
- **Requêtes paramétrées** : toutes les requêtes SQL sont paramétrées.
- **Échecs explicites** : projet inconnu, ambiguïté de nom ou période invalide
  produisent une erreur claire et un code de sortie non nul.
