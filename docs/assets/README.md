# Captures d'écran (`docs/assets/`)

Ce dossier est destiné aux captures d'écran de la TUI utilisées par le
`README.md`. **Aucune image binaire n'est versionnée par défaut** : générez la
vôtre en suivant les instructions ci-dessous.

## Image attendue

- Nom : `mnemo-tui.png`
- Contenu : la TUI « ops dashboard » de `mnemo` (barre de commande, synthèse,
  liste des commandes, panneau de détails).

Une fois `docs/assets/mnemo-tui.png` présent, remplacez le bloc texte de la
section « Aperçu » du `README.md` par :

```markdown
![TUI mnemo](docs/assets/mnemo-tui.png)
```

## Générer la capture

### Option A : capture du terminal (recommandée)

1. Lancer la TUI avec quelques commandes en base :

   ```bash
   mnemo tui
   ```

2. Capturer la fenêtre du terminal avec l'outil de votre environnement :
   - GNOME : `gnome-screenshot -w`
   - KDE : Spectacle (`Print`)
   - WSL : outil Capture d'écran de Windows (`Win+Shift+S`)

3. Enregistrer le fichier sous `docs/assets/mnemo-tui.png`.

### Option B : rendu texte reproductible

Un aperçu **texte** fidèle est déjà inclus dans le `README.md` (section
« Aperçu »). Il est produit par le rendu réel de la TUI dans un backend de test
Ratatui (largeur 96, hauteur 24) ; il peut servir de base à une capture stylée
via un outil de type [`carbon`](https://carbon.now.sh/) ou
[`freeze`](https://github.com/charmbracelet/freeze) :

```bash
# Exemple avec freeze (à installer séparément) :
freeze apercu.txt --output docs/assets/mnemo-tui.png
```

## Bonnes pratiques

- Préférer un thème de terminal sombre (la palette mnemo est pensée pour fond
  sombre).
- Largeur d'au moins ~100 colonnes pour afficher la synthèse et le panneau de
  détails.
- Éviter d'inclure des informations sensibles (chemins, noms d'hôtes réels).
