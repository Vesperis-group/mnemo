# Onboarding et expérience utilisateur

Ce document décrit le parcours de premier démarrage de mnemo, l'assistant
interactif `mnemo init --wizard`, la génération des complétions shell et la page
de manuel. L'objectif est une prise en main rapide sans jamais effectuer
d'action destructive.

## Principes

- **Local d'abord.** Toutes les données restent sur la machine : configuration,
  base SQLite et sauvegardes. Aucune synchronisation distante, aucune
  télémétrie.
- **Bash en priorité.** L'enregistrement automatique repose sur un hook installé
  dans `~/.bashrc`. Les autres shells utilisent mnemo via l'import et les
  complétions.
- **Jamais destructif.** L'assistant ne supprime ni ne purge aucune donnée. En
  cas de doute, il ne fait rien et explique pourquoi.

## Premier démarrage

### Initialisation simple

```bash
mnemo init
```

Crée la configuration et la base si nécessaire, resserre leurs permissions, puis
affiche le bloc Bash à ajouter à `~/.bashrc`. Cette commande est idempotente :
la relancer ne casse rien.

### Assistant interactif

```bash
mnemo init --wizard
```

L'assistant déroule les étapes suivantes :

1. **Aperçu de l'installation.** Affiche l'emplacement de la configuration, de la
   base SQLite, du binaire et du dossier de sauvegardes.
2. **Initialisation.** Crée la configuration et la base si elles n'existent pas
   encore.
3. **Intégration Bash.** Propose d'ajouter le hook à `~/.bashrc`. Une sauvegarde
   du fichier est créée et le bloc n'est jamais dupliqué.
4. **Import optionnel.** Si `~/.bash_history` existe, propose de l'importer. Les
   commandes sensibles et les doublons sont ignorés selon la configuration.
5. **Diagnostic.** Propose de lancer `mnemo doctor`. Une erreur de diagnostic
   n'interrompt pas l'onboarding déjà réalisé.

Chaque étape pose une question oui/non avec une valeur par défaut sûre indiquée
entre crochets (`[O/n]` ou `[o/N]`).

### Mode non interactif

En contexte non interactif (script, CI, sortie redirigée), l'assistant refuse de
s'exécuter pour ne pas prendre de décision silencieuse :

```bash
mnemo init --wizard
# Erreur : un terminal interactif est requis.
```

Pour automatiser malgré tout en acceptant les choix sûrs par défaut, ajoutez
`--yes` :

```bash
mnemo init --wizard --yes
```

`--yes` répond à chaque question par sa valeur par défaut. Aucune suppression
n'est jamais effectuée.

## Complétions shell

`mnemo completions <shell>` écrit le script de complétion sur la sortie standard.
mnemo ne modifie jamais vos fichiers shell : vous redirigez vous-même la sortie.

### Bash

```bash
# Session courante
source <(mnemo completions bash)

# Installation persistante
mnemo completions bash > ~/.local/share/bash-completion/completions/mnemo
```

### Zsh

```bash
mnemo completions zsh > ~/.zsh/completions/_mnemo
# Assurez-vous que ~/.zsh/completions figure dans $fpath, puis : compinit
```

### Fish

```bash
mnemo completions fish > ~/.config/fish/completions/mnemo.fish
```

Un shell non supporté produit une erreur claire listant les valeurs possibles
(`bash`, `zsh`, `fish`).

## Page de manuel

La page de manuel est fournie au format troff :

```bash
man ./docs/man/mnemo.1
```

Elle couvre les commandes principales, les fichiers utilisés, les considérations
de sécurité et des exemples.
