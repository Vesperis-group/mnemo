use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "mnemo",
    version,
    about = "Navigation et recherche dans l'historique Bash",
    long_about = None,
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Initialise la configuration et la base de données.
    Init,

    /// Importe l'historique Bash (~/.bash_history par défaut) dans la base.
    Import {
        /// Fichier d'historique à importer.
        #[arg(long)]
        file: Option<PathBuf>,
    },

    /// Ajoute une commande dans la base.
    Add {
        /// Commande à enregistrer.
        #[arg(long)]
        cmd: String,
        /// Répertoire de travail (défaut : répertoire courant).
        #[arg(long)]
        cwd: Option<String>,
        /// Code de sortie de la commande.
        #[arg(long = "exit-code", default_value_t = 0)]
        exit_code: i64,
    },

    /// Ouvre l'interface TUI interactive de recherche.
    Search {
        /// Requête initiale (positionnelle, optionnelle).
        query: Option<String>,
        /// Requête explicite (équivalent à l'argument positionnel).
        #[arg(long = "query", value_name = "TEXTE", conflicts_with = "query")]
        query_opt: Option<String>,
        /// Mode non interactif : imprime les résultats sur stdout sans TUI.
        #[arg(long)]
        print: bool,
        /// Nombre maximal de résultats affichés en mode --print.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Filtre sur un projet Git (nom du dossier racine ou chemin git_root).
        #[arg(long, value_name = "NOM")]
        project: Option<String>,
        /// Filtre sur une branche Git.
        #[arg(long, value_name = "BRANCHE")]
        branch: Option<String>,
    },

    /// Affiche le snippet d'intégration Bash à ajouter dans ~/.bashrc.
    Bashrc,

    /// Applique les migrations de schéma SQLite en attente.
    Migrate,

    /// Affiche des statistiques d'usage (texte simple).
    Stats {
        /// Filtre sur un projet Git (nom du dossier racine ou chemin git_root).
        #[arg(long, value_name = "NOM")]
        project: Option<String>,
        /// Filtre sur une branche Git.
        #[arg(long, value_name = "BRANCHE")]
        branch: Option<String>,
        /// Produit une sortie JSON exploitable.
        #[arg(long)]
        json: bool,
    },

    /// Diagnostique l'installation locale de mnemo.
    Doctor {
        /// Répare les éléments manquants (config, base, bloc .bashrc).
        #[arg(long)]
        fix: bool,
        /// Produit une sortie JSON exploitable.
        #[arg(long)]
        json: bool,
    },

    /// Gère la configuration locale de mnemo.
    Config {
        #[command(subcommand)]
        action: ConfigCommand,
    },

    /// Affiche des informations détaillées de version et de build.
    Version,
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Gère la liste des commandes ignorées dans `mnemo stats`.
    StatsIgnore {
        #[command(subcommand)]
        action: StatsIgnoreCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum StatsIgnoreCommand {
    /// Ajoute une commande à la liste ignorée du Top commandes.
    Add {
        /// Nom de commande (ex: `create_dir`).
        name: String,
    },
    /// Retire une commande de la liste ignorée.
    Remove {
        /// Nom de commande (ex: `create_dir`).
        name: String,
    },
    /// Affiche les commandes actuellement ignorées.
    List,
}
