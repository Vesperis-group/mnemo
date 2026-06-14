# Compatibilité et stabilité

Ce document décrit les environnements pris en charge par `mnemo` et les
garanties de stabilité visées, en particulier à partir de la version **1.0**.

> Avant la v1.0, la surface CLI peut encore évoluer (ajustements d'options,
> reformulation de messages). Les changements notables sont consignés dans le
> `CHANGELOG.md`.

## Environnements pris en charge

| Élément | État | Détail |
| --- | --- | --- |
| Linux x86-64 | ✅ Pris en charge | Cible principale, validée en CI |
| WSL2 (Ubuntu/Debian) | ✅ Pris en charge | Identique à Linux |
| Bash | ✅ Pris en charge | Hook d'enregistrement `PROMPT_COMMAND` |
| Binaire de release | ✅ `x86_64-unknown-linux-musl` | Statique, sans dépendance glibc |
| macOS | ⚠️ Best-effort | Compile depuis les sources, non testé en continu |
| Zsh / Fish | ⚠️ Partiel | Recherche utilisable ; hook d'enregistrement non fourni |
| Windows natif | ❌ Non visé | Utiliser WSL |

## Versionnage sémantique (à partir de la v1.0)

mnemo suit le [versionnage sémantique](https://semver.org/lang/fr/) :

- **MAJEURE** : changement incompatible - suppression d'une commande ou d'une
  option, rupture d'un format de sortie stable, changement de comportement par
  défaut destructif.
- **MINEURE** : ajout rétrocompatible - nouvelle commande, nouvelle option,
  nouveau champ dans une sortie JSON.
- **CORRECTIVE** : correction de bug ou de sécurité sans rupture d'interface.

## Surface CLI stable

Les commandes suivantes sont considérées comme stables et resteront disponibles
avec un comportement rétrocompatible à partir de la v1.0 :

`init`, `add`, `import`, `search`, `tui`, `stats`, `export`, `list`, `prune`,
`backup`, `restore`, `maintenance`, `config`, `doctor`, `version`, `update`,
`upgrade`, `uninstall`.

Toute évolution incompatible de ces commandes implique une version majeure.

## Sorties JSON

Les sorties JSON sont destinées aux scripts et à la CI. À partir de la v1.0,
leur structure est **stable et versionnée** ; les évolutions se font par
**ajout de champs** (jamais par suppression ou renommage silencieux) :

- `mnemo search --print --json`
- `mnemo stats --json`
- `mnemo doctor --json`
- `mnemo export --format json` (option `--gzip`)
- `mnemo list --json` (le cas échéant)

Les consommateurs doivent ignorer les champs inconnus pour rester compatibles
avec les versions futures.

## Migrations de base de données

- Au démarrage, mnemo applique automatiquement les migrations nécessaires pour
  amener la base au schéma courant.
- Si la base a été créée par une **version plus récente** de mnemo (schéma
  inconnu), mnemo **refuse de l'utiliser** plutôt que de risquer une corruption.
- Les sauvegardes (`mnemo backup`) permettent de revenir à un état antérieur.

## Configuration

Le fichier de configuration TOML est rétrocompatible : les clés inconnues sont
ignorées et les valeurs absentes prennent une valeur par défaut documentée.
Une configuration invalide est signalée par `mnemo config validate` et provoque
un code de sortie non nul.

## Codes de sortie

| Contexte | Code |
| --- | --- |
| Succès d'une commande | `0` |
| `doctor` sain ou avec avertissements seulement | `0` |
| `doctor` avec au moins une erreur bloquante | `1` |
| Erreur CLI (arguments invalides, échec d'exécution) | non nul |
| Configuration invalide (`config validate`) | non nul |
| Vérification de signature stricte en échec (`--require-signature`) | non nul |
| Action destructive refusée (sans `--yes` en mode non interactif) | non nul |

## Dépréciations

Les fonctionnalités amenées à être retirées seront :

1. annoncées dans le `CHANGELOG.md` ;
2. signalées par un avertissement à l'exécution lorsque c'est possible ;
3. retirées au plus tôt à la version **majeure** suivante.
