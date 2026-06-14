use clap::{Parser, Subcommand};
use std::path::PathBuf;

use crate::export::ExportFormat;

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

    /// Ouvre la TUI avancée (interface interactive principale).
    Tui {
        /// Requête initiale (positionnelle, optionnelle).
        query: Option<String>,
        /// Filtre initial sur un projet Git (nom du dossier racine).
        #[arg(long, value_name = "NOM")]
        project: Option<String>,
        /// Filtre initial sur une branche Git.
        #[arg(long, value_name = "BRANCHE")]
        branch: Option<String>,
        /// Filtre initial sur un répertoire de travail.
        #[arg(long, value_name = "CHEMIN")]
        cwd: Option<String>,
        /// N'affiche que les commandes en échec (exit_code ≠ 0).
        #[arg(long)]
        failed: bool,
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

    /// Crée une sauvegarde locale complète (archive .tar.gz).
    Backup {
        /// Dossier de destination (défaut : ~/.local/share/mnemo/backups/).
        #[arg(long, value_name = "DOSSIER")]
        output: Option<PathBuf>,
        /// Produit une sortie JSON exploitable.
        #[arg(long)]
        json: bool,
    },

    /// Restaure une sauvegarde (.tar.gz) après vérification.
    Restore {
        /// Chemin de l'archive de sauvegarde.
        archive: PathBuf,
        /// Montre ce qui serait fait sans rien modifier.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Confirme la restauration sans question interactive.
        #[arg(long)]
        yes: bool,
    },

    /// Exporte les commandes en JSON ou CSV.
    Export {
        /// Format de sortie.
        #[arg(long, value_enum)]
        format: ExportFormat,
        /// Filtre sur un projet Git (nom du dossier racine ou chemin git_root).
        #[arg(long, value_name = "NOM")]
        project: Option<String>,
        /// Filtre sur une branche Git.
        #[arg(long, value_name = "BRANCHE")]
        branch: Option<String>,
        /// Fichier de sortie (défaut : stdout).
        #[arg(long, value_name = "FICHIER")]
        output: Option<PathBuf>,
    },

    /// Affiche les dernières commandes avec leurs IDs.
    List {
        /// Nombre de commandes affichées (défaut : 20).
        #[arg(long)]
        limit: Option<usize>,
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

    /// Supprime une commande par son ID (après confirmation).
    Delete {
        /// Identifiant de la commande à supprimer.
        id: i64,
        /// Montre la commande ciblée sans la supprimer.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Confirme la suppression sans question interactive.
        #[arg(long)]
        yes: bool,
    },

    /// Nettoie les commandes plus anciennes qu'une durée donnée.
    Prune {
        /// Durée d'ancienneté (ex : 30d, 12w, 6m, 1y).
        #[arg(long = "older-than", value_name = "DURÉE")]
        older_than: String,
        /// Filtre sur un projet Git (nom du dossier racine ou chemin git_root).
        #[arg(long, value_name = "NOM")]
        project: Option<String>,
        /// Filtre sur une branche Git.
        #[arg(long, value_name = "BRANCHE")]
        branch: Option<String>,
        /// Montre ce qui serait supprimé sans rien modifier.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Confirme le nettoyage sans question interactive.
        #[arg(long)]
        yes: bool,
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
