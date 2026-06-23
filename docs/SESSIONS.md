# Sessions de travail

`mnemo session` permet de lister, consulter et exporter des **sessions de
travail** reconstituées à partir de l'historique déjà stocké. Une session
regroupe les commandes d'un même shell interactif, ce qui est utile pour
documenter une intervention, un TP, un audit ou une procédure.

## Qu'est-ce qu'une session ?

Une session est l'ensemble des commandes partageant un même `session_id`. Cet
identifiant est défini par la variable d'environnement `MNEMO_SESSION_ID`,
capturée au moment de l'enregistrement par l'intégration shell.

Depuis l'intégration Bash installée par `mnemo init`, chaque shell interactif
reçoit automatiquement un `MNEMO_SESSION_ID` stable pour toute sa durée de vie :

```bash
if [ -z "${MNEMO_SESSION_ID:-}" ]; then
    export MNEMO_SESSION_ID="$(date +%Y%m%dT%H%M%S)-$$"
fi
```

### Commandes sans session

Les commandes **importées** depuis un ancien `~/.bash_history`, ou enregistrées
avant la mise à jour de l'intégration shell, ne possèdent pas de `session_id`.
Elles ne sont rattachées à aucune session et sont donc ignorées par
`mnemo session`. C'est volontaire : mnemo ne fabrique jamais de session
artificielle.

Pour capturer les prochaines sessions, réinstallez l'intégration shell
(`mnemo init`, ou le bloc fourni par `mnemo bashrc`) puis ouvrez un nouveau
shell.

### Mettre à niveau une intégration existante

Les installations antérieures à `mnemo session` contiennent un bloc Bash qui ne
définit pas `MNEMO_SESSION_ID`. Pour le mettre à niveau sans réinstaller :

```bash
mnemo shell upgrade
source ~/.bashrc
```

`mnemo shell upgrade` détecte l'état du bloc présent dans `~/.bashrc` :

- **aucun bloc** : rien n'est modifié ; lancez `mnemo init` pour l'installer ;
- **bloc obsolète** : le `.bashrc` est sauvegardé puis le seul bloc mnemo est
  remplacé par la version courante. Le reste du fichier n'est jamais touché ;
- **bloc à jour** : aucune modification.

`mnemo doctor` signale également un bloc obsolète et propose la même commande.
Les shells déjà ouverts doivent être rechargés avec `source ~/.bashrc`. Les
commandes anciennes sans `session_id` restent consultables via les autres
commandes (`mnemo list`, `mnemo search`), mais ne sont pas regroupées en
session.

## Lister les sessions

```bash
mnemo session list
mnemo session list --limit 20
```

Affiche les sessions de la plus récente à la plus ancienne, avec le nombre de
commandes, les bornes temporelles et le projet associé.

## Consulter une session

```bash
mnemo session show <session_id>
mnemo session show <session_id> --limit 100
```

Affiche l'en-tête de la session (projet, branche, nombre de commandes, bornes)
puis les commandes dans l'ordre chronologique. Le code de sortie n'est indiqué
que pour les commandes en échec, afin de garder la liste lisible.

## Exporter une session

```bash
mnemo session export <session_id>
mnemo session export --last
mnemo session export <session_id> --format markdown
mnemo session export <session_id> --format json
mnemo session export --last --output work-session.md
mnemo session export --last --output work-session.md --force
```

Comportement :

- format `markdown` par défaut, `json` disponible ;
- sortie sur stdout par défaut, ou dans le fichier indiqué par `--output` ;
- un fichier existant n'est jamais écrasé sans `--force` ;
- `--last` cible la session la plus récente (erreur claire si aucune session
  n'existe).

### Format Markdown

Le document Markdown est directement réutilisable : métadonnées de session, bloc
de commandes prêt à copier, puis tableau chronologique détaillé. Les commandes
contenant des backticks ou des barres verticales sont échappées pour ne pas
casser la structure.

### Format JSON

Le JSON est stable et lisible : métadonnées de session puis liste ordonnée des
commandes (horodatage, répertoire, code de sortie, branche, commande). Pratique
pour un traitement automatisé.

## Confidentialité

L'export réutilise les données déjà filtrées à l'enregistrement : les commandes
considérées sensibles n'ont jamais été stockées et n'apparaissent donc pas dans
les sessions. `mnemo session` ne refait pas de scan et n'expose aucune donnée
hors de la machine.
