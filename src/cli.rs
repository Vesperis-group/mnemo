use clap::{Parser, Subcommand, ValueEnum};
use clap_complete::Shell;
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
    Init {
        /// Lance l'assistant d'onboarding interactif (intégration Bash, import,
        /// diagnostic). Toutes les actions proposées sont non destructives.
        #[arg(long)]
        wizard: bool,
        /// En mode `--wizard` non interactif, accepte les choix sûrs par défaut
        /// sans rien supprimer ni purger.
        #[arg(long)]
        yes: bool,
    },

    /// Génère un script de complétion shell sur stdout (bash, zsh, fish).
    ///
    /// mnemo n'écrit jamais dans vos fichiers shell : redirigez la sortie vers
    /// l'emplacement adéquat (voir `docs/UX_ONBOARDING.md`).
    Completions {
        /// Shell cible.
        #[arg(value_enum)]
        shell: CompletionShell,
    },

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
        /// Filtre sur un code de sortie exact (ex : 0, 1, 127).
        #[arg(long = "exit-code", value_name = "CODE")]
        exit_code: Option<i64>,
        /// N'affiche que les commandes en échec (exit_code ≠ 0).
        #[arg(long, conflicts_with = "exit_code")]
        failed: bool,
        /// Limite l'âge des résultats (durée `7d`/`2w`/`3m`/`1y` ou date `AAAA-MM-JJ`).
        #[arg(long, value_name = "DURÉE|DATE")]
        since: Option<String>,
        /// N'affiche que les commandes antérieures à une date (`AAAA-MM-JJ`).
        #[arg(long, value_name = "DATE")]
        before: Option<String>,
        /// Filtre sur un répertoire de travail exact.
        #[arg(long, value_name = "CHEMIN")]
        cwd: Option<String>,
        /// Filtre sur un shell exact (ex : bash, zsh).
        #[arg(long, value_name = "SHELL")]
        shell: Option<String>,
        /// Avec --print, produit une sortie JSON stable.
        #[arg(long)]
        json: bool,
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

    /// Gère l'intégration shell installée dans ~/.bashrc.
    Shell {
        #[command(subcommand)]
        action: ShellCommand,
    },

    /// Applique les migrations de schéma SQLite en attente.
    Migrate,

    /// Affiche des statistiques d'usage (texte simple).
    Stats {
        /// Filtre sur un projet Git (nom du dossier racine, chemin git_root, ou `current`).
        #[arg(long, value_name = "NOM")]
        project: Option<String>,
        /// Filtre sur une branche Git.
        #[arg(long, value_name = "BRANCHE")]
        branch: Option<String>,
        /// Limite la fenêtre d'analyse (durée `7d`/`2w`/`3m`/`1y` ou date `AAAA-MM-JJ`).
        #[arg(long, value_name = "DURÉE|DATE")]
        since: Option<String>,
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
        /// Compresse la sortie en gzip (`.json.gz` / `.csv.gz`).
        #[arg(long)]
        gzip: bool,
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

    /// Vérifie si une nouvelle version est disponible (sans rien installer).
    ///
    /// En terminal interactif, si une mise à jour existe, propose de lancer
    /// `mnemo upgrade` immédiatement (réponse par défaut : non). En mode non
    /// interactif (CI, script, cron, pipe), reste une simple vérification.
    /// `--upgrade` enchaîne directement l'installation quand une mise à jour est
    /// disponible ; combiné à `--yes`, il permet un upgrade automatisé.
    /// `--require-signature` rend la vérification Sigstore (cosign) obligatoire
    /// lors de l'upgrade enchaîné.
    Update {
        /// Sortie au format JSON (vérification seule, sans proposition).
        #[arg(long)]
        json: bool,
        /// Si une mise à jour est disponible, lance directement `mnemo upgrade`.
        #[arg(long)]
        upgrade: bool,
        /// Avec `--upgrade`, installe sans confirmation interactive.
        #[arg(long)]
        yes: bool,
        /// Avec `--upgrade`, exige une signature Sigstore valide (cosign).
        #[arg(long = "require-signature")]
        require_signature: bool,
    },

    /// Télécharge et installe la dernière version stable (remplace le binaire).
    Upgrade {
        /// Montre ce qui serait fait sans rien télécharger ni remplacer.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Confirme l'installation sans question interactive.
        #[arg(long)]
        yes: bool,
        /// Force une version précise (ex : v0.5.0) au lieu de la dernière.
        #[arg(long, value_name = "VERSION")]
        version: Option<String>,
        /// Force un triplet cible (ex : aarch64-unknown-linux-musl).
        #[arg(long, value_name = "CIBLE")]
        target: Option<String>,
        /// Exige une signature Sigstore valide (cosign requis) avant d'installer.
        #[arg(long = "require-signature")]
        require_signature: bool,
    },

    /// Désinstalle mnemo : binaire + intégration shell. Conserve les données.
    Uninstall {
        /// Montre ce qui serait supprimé sans rien modifier.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Confirme la désinstallation sans question interactive.
        #[arg(long)]
        yes: bool,
        /// Supprime AUSSI la configuration, la base et les sauvegardes.
        #[arg(long)]
        purge: bool,
    },

    /// Inspecte le projet courant et les projets connus de l'historique.
    Project {
        #[command(subcommand)]
        action: ProjectCommand,
    },

    /// Maintenance de l'historique (nettoyage automatique configurable).
    Maintenance {
        #[command(subcommand)]
        action: MaintenanceCommand,
    },

    /// Navigue, consulte et exporte des sessions de travail.
    ///
    /// Une session regroupe les commandes partageant un même `session_id`,
    /// capturé par l'intégration shell (`MNEMO_SESSION_ID`). Les commandes
    /// importées ou enregistrées sans cet identifiant ne sont pas rattachées à
    /// une session.
    Session {
        #[command(subcommand)]
        action: SessionCommand,
    },

    /// Analyse et redacte les secrets présents dans l'historique déjà stocké.
    ///
    /// `scan` repère les commandes potentiellement sensibles et les affiche
    /// toujours sous forme redactée. `redact` les nettoie en place (dry-run par
    /// défaut, sauvegarde obligatoire avant toute écriture). Aucun secret n'est
    /// jamais affiché en clair.
    Secrets {
        #[command(subcommand)]
        action: SecretsCommand,
    },
}

/// Regroupe les options de `mnemo search` pour éviter une fonction à trop
/// d'arguments (filtres combinables passés en un bloc).
#[derive(Debug, Default)]
pub struct SearchArgs {
    pub query: Option<String>,
    pub print: bool,
    pub limit: usize,
    pub project: Option<String>,
    pub branch: Option<String>,
    pub exit_code: Option<i64>,
    pub failed: bool,
    pub since: Option<String>,
    pub before: Option<String>,
    pub cwd: Option<String>,
    pub shell: Option<String>,
    pub json: bool,
}

/// Shells supportés par `mnemo completions`. Limité volontairement à bash, zsh
/// et fish (un shell inconnu produit une erreur claire de clap). L'enregistrement
/// automatique du hook reste, lui, Bash-first.
#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum CompletionShell {
    Bash,
    Zsh,
    Fish,
}

impl CompletionShell {
    /// Convertit vers le générateur `clap_complete` correspondant.
    pub fn generator(self) -> Shell {
        match self {
            CompletionShell::Bash => Shell::Bash,
            CompletionShell::Zsh => Shell::Zsh,
            CompletionShell::Fish => Shell::Fish,
        }
    }
}

#[derive(Subcommand, Debug)]
pub enum ConfigCommand {
    /// Affiche la configuration effective (valeurs par défaut incluses).
    Show,
    /// Affiche le chemin du fichier de configuration.
    Path,
    /// Ouvre la configuration dans l'éditeur ($EDITOR, sinon nano/vi).
    Edit,
    /// Vérifie la validité du fichier de configuration.
    Validate,
    /// Gère la liste des commandes ignorées dans `mnemo stats`.
    StatsIgnore {
        #[command(subcommand)]
        action: StatsIgnoreCommand,
    },
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    /// Affiche le projet détecté pour le répertoire courant.
    Current,
    /// Liste les projets connus de l'historique.
    List,
}

#[derive(Subcommand, Debug)]
pub enum MaintenanceCommand {
    /// Affiche l'état de la maintenance et ce qui serait nettoyé.
    Status,
    /// Exécute le nettoyage configuré.
    Run {
        /// Montre ce qui serait supprimé sans rien modifier.
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Confirme le nettoyage sans question interactive.
        #[arg(long)]
        yes: bool,
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

#[derive(Subcommand, Debug)]
pub enum ShellCommand {
    /// Met à niveau l'intégration Bash installée dans ~/.bashrc.
    ///
    /// Remplace un bloc obsolète par la version courante (capture de
    /// `MNEMO_SESSION_ID` pour `mnemo session`), après sauvegarde et sans
    /// toucher au reste du fichier. Sans bloc installé, propose `mnemo init`.
    Upgrade,
}

/// Format d'export d'une session (`mnemo session export`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum SessionFormat {
    Markdown,
    Json,
}

#[derive(Subcommand, Debug)]
pub enum SessionCommand {
    /// Liste les sessions connues, de la plus récente à la plus ancienne.
    List {
        /// Nombre maximal de sessions affichées.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },
    /// Affiche les commandes d'une session, dans l'ordre chronologique.
    Show {
        /// Identifiant de session (voir `mnemo session list`).
        session_id: String,
        /// Nombre maximal de commandes affichées.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
    },
    /// Exporte une session en Markdown (défaut) ou JSON.
    Export {
        /// Identifiant de session à exporter (incompatible avec `--last`).
        #[arg(value_name = "SESSION_ID", conflicts_with = "last")]
        session_id: Option<String>,
        /// Cible la session la plus récente au lieu d'un identifiant explicite.
        #[arg(long)]
        last: bool,
        /// Format de sortie (`markdown` par défaut).
        #[arg(long, value_enum, default_value = "markdown")]
        format: SessionFormat,
        /// Fichier de sortie (défaut : stdout).
        #[arg(long, value_name = "FICHIER")]
        output: Option<PathBuf>,
        /// Autorise l'écrasement d'un fichier de sortie existant.
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum SecretsCommand {
    /// Repère les commandes potentiellement sensibles (lecture seule).
    ///
    /// Les commandes sont toujours affichées sous forme redactée ; aucun secret
    /// n'apparaît en clair. N'effectue aucune modification.
    Scan {
        /// Nombre maximal de résultats affichés.
        #[arg(long, value_name = "N")]
        limit: Option<usize>,
        /// Sortie JSON (sans valeurs sensibles).
        #[arg(long)]
        json: bool,
    },
    /// Redacte en place les commandes sensibles déjà stockées.
    ///
    /// Dry-run par défaut : sans `--apply`, rien n'est modifié. Avec `--apply`,
    /// une sauvegarde est créée avant toute écriture et seule la colonne
    /// `command` est mise à jour.
    Redact {
        /// Montre ce qui serait redacté sans rien modifier (comportement par
        /// défaut, accepté explicitement).
        #[arg(long = "dry-run")]
        dry_run: bool,
        /// Applique réellement la redaction (sinon dry-run).
        #[arg(long)]
        apply: bool,
        /// Confirme la redaction sans question interactive.
        #[arg(long)]
        yes: bool,
        /// Force une sauvegarde avant redaction (toujours effectuée avec
        /// `--apply`, ce drapeau le rend explicite).
        #[arg(long)]
        backup: bool,
    },
}
