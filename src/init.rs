//! Commande `mnemo init` : initialisation et assistant d'onboarding.
//!
//! `mnemo init` crée la configuration et la base si nécessaire, puis affiche le
//! snippet Bash à installer. `mnemo init --wizard` ajoute un accompagnement
//! interactif (intégration Bash, import de l'historique, diagnostic) en
//! réutilisant la logique existante de `doctor`, `shell` et `importer`.
//!
//! Invariant de sûreté : le wizard est strictement non destructif. Il n'efface
//! et ne purge jamais de données. En contexte non interactif sans `--yes`, il
//! refuse de s'exécuter plutôt que de prendre des décisions silencieuses.

use anyhow::Result;
use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};

use crate::{backup, config, db, doctor, importer, shell};

/// Point d'entrée de `mnemo init`.
pub fn run(wizard: bool, assume_yes: bool) -> Result<()> {
    if wizard {
        run_wizard(assume_yes)
    } else {
        run_basic()
    }
}

/// Point d'entrée de `mnemo shell upgrade`.
///
/// Met à niveau le bloc d'intégration Bash existant vers la version courante
/// (capture de `MNEMO_SESSION_ID`). Non destructif : sauvegarde systématique,
/// aucun bloc créé s'il est absent.
pub fn run_shell_upgrade() -> Result<()> {
    let Some(bashrc) = bashrc_path() else {
        eprintln!("Répertoire personnel introuvable : mise à niveau impossible.");
        std::process::exit(1);
    };

    match shell::upgrade_block(&bashrc)? {
        shell::ShellUpgrade::NotInstalled => {
            println!(
                "Aucune intégration Bash mnemo détectée dans {}.",
                display_home(&bashrc)
            );
            println!("Lancez `mnemo init` pour l'installer.");
        }
        shell::ShellUpgrade::AlreadyCurrent => {
            println!(
                "Intégration Bash mnemo déjà à jour dans {} : aucun changement.",
                display_home(&bashrc)
            );
        }
        shell::ShellUpgrade::Upgraded { backup } => {
            println!("Intégration Bash mnemo détectée : obsolète.");
            println!("Sauvegarde créée : {}", display_home(&backup));
            println!("Bloc d'intégration Bash mnemo mis à niveau.");
            println!("Prochaine étape : source ~/.bashrc");
        }
    }
    Ok(())
}

/// Initialisation simple (comportement historique de `mnemo init`).
fn run_basic() -> Result<()> {
    ensure_config_and_db()?;

    println!();
    println!("Ajoutez ces lignes à votre ~/.bashrc :");
    println!("------------------------------------------------------------");
    print!("{}", shell::bashrc_snippet());
    println!("------------------------------------------------------------");
    println!("Puis rechargez : source ~/.bashrc");
    Ok(())
}

/// Assistant d'onboarding interactif.
fn run_wizard(assume_yes: bool) -> Result<()> {
    let interactive = io::stdin().is_terminal();
    if !interactive && !assume_yes {
        eprintln!(
            "mnemo init --wizard nécessite un terminal interactif. En contexte non \
             interactif (script, CI, pipe), relancez avec --yes pour accepter les \
             choix sûrs par défaut (jamais de suppression)."
        );
        std::process::exit(1);
    }

    println!("Bienvenue dans mnemo.");
    println!();
    println!(
        "mnemo va vérifier votre installation locale, initialiser la configuration \
         si nécessaire, puis vous proposer l'intégration Bash et l'import de votre \
         historique existant. Aucune donnée n'est jamais supprimée."
    );
    println!();

    print_install_overview()?;
    println!();

    ensure_config_and_db()?;
    println!();

    setup_bash_integration(interactive)?;
    println!();

    match bash_history_path() {
        Some(hist) if hist.exists() => {
            let question = format!("Importer {} maintenant ?", display_home(&hist));
            if ask(&question, false, interactive)? {
                import_history(&hist)?;
            }
        }
        Some(hist) => {
            println!(
                "Historique {} introuvable : import ignoré.",
                display_home(&hist)
            );
        }
        None => {}
    }
    println!();

    if ask("Lancer mnemo doctor ?", true, interactive)? {
        println!();
        // Le diagnostic est optionnel : une erreur ne doit pas faire échouer
        // l'onboarding déjà réalisé.
        if let Err(err) = doctor::run(false, false) {
            eprintln!("Diagnostic ignoré : {err}");
        }
    }

    print_next_steps();
    Ok(())
}

/// Crée la configuration et la base si nécessaire (idempotent) et resserre les
/// permissions. Affiche les chemins concernés, comme le `mnemo init` historique.
fn ensure_config_and_db() -> Result<()> {
    let cfg_path = config::config_path()?;
    if cfg_path.exists() {
        println!("Configuration existante : {}", cfg_path.display());
    } else {
        config::Config::default().save(&cfg_path)?;
        println!("Configuration créée : {}", cfg_path.display());
    }
    // Idempotent : resserre les permissions même si la config préexistait.
    config::harden_file(&cfg_path);

    let db_path = config::db_path()?;
    db::open(&db_path)?;
    config::harden_file(&db_path);
    println!("Base de données : {}", db_path.display());
    Ok(())
}

/// Étape d'intégration Bash du wizard : installe le bloc s'il est absent, le met
/// à niveau s'il est obsolète, ou ne fait rien s'il est déjà à jour. Toujours
/// non destructif (sauvegarde avant toute écriture).
fn setup_bash_integration(interactive: bool) -> Result<()> {
    let Some(bashrc) = bashrc_path() else {
        eprintln!("Répertoire personnel introuvable : intégration Bash ignorée.");
        return Ok(());
    };
    let content = std::fs::read_to_string(&bashrc).unwrap_or_default();

    match shell::block_state(&content) {
        shell::BlockState::Absent => {
            if ask(
                "Ajouter l'intégration Bash dans ~/.bashrc ?",
                true,
                interactive,
            )? {
                add_bash_integration()?;
            }
        }
        shell::BlockState::Legacy => {
            if ask(
                "Mettre à jour l'intégration Bash existante (active les sessions) ?",
                true,
                interactive,
            )? {
                upgrade_bash_integration(&bashrc)?;
            }
        }
        shell::BlockState::Current => {
            println!(
                "Intégration Bash déjà à jour dans {} : aucun changement.",
                display_home(&bashrc)
            );
        }
    }
    Ok(())
}

/// Ajoute le bloc d'intégration Bash au `.bashrc` (sauvegarde automatique,
/// jamais de doublon) via [`shell::install_block`].
fn add_bash_integration() -> Result<()> {
    let Some(bashrc) = bashrc_path() else {
        eprintln!("Répertoire personnel introuvable : intégration Bash ignorée.");
        return Ok(());
    };
    if shell::install_block(&bashrc)? {
        println!(
            "Intégration Bash ajoutée à {} (sauvegarde du fichier créée).",
            display_home(&bashrc)
        );
    } else {
        println!(
            "Intégration Bash déjà présente dans {} : aucun changement.",
            display_home(&bashrc)
        );
    }
    Ok(())
}

/// Met à niveau un bloc d'intégration Bash obsolète via [`shell::upgrade_block`].
fn upgrade_bash_integration(bashrc: &Path) -> Result<()> {
    match shell::upgrade_block(bashrc)? {
        shell::ShellUpgrade::Upgraded { backup } => {
            println!(
                "Intégration Bash mise à niveau dans {} (sauvegarde {} créée).",
                display_home(bashrc),
                display_home(&backup)
            );
        }
        shell::ShellUpgrade::AlreadyCurrent => {
            println!("Intégration Bash déjà à jour : aucun changement.");
        }
        shell::ShellUpgrade::NotInstalled => {
            // État inattendu (le bloc a disparu entre-temps) : installation propre.
            add_bash_integration()?;
        }
    }
    Ok(())
}

/// Importe l'historique Bash pointé par `path` dans la base.
fn import_history(path: &Path) -> Result<()> {
    let cfg = config::Config::load()?;
    let conn = db::open(&config::db_path()?)?;
    let stats = importer::import_bash_history(&conn, path, &cfg)?;
    println!("Import depuis {}", display_home(path));
    println!("  Importées          : {}", stats.imported);
    println!("  Sensibles ignorées : {}", stats.skipped_sensitive);
    println!("  Doublons ignorés   : {}", stats.skipped_duplicate);
    Ok(())
}

/// Affiche l'emplacement des données locales gérées par mnemo.
fn print_install_overview() -> Result<()> {
    println!("Données :");
    println!(
        "  Configuration : {}",
        display_home(&config::config_path()?)
    );
    println!("  Base SQLite   : {}", display_home(&config::db_path()?));
    if let Ok(exe) = std::env::current_exe() {
        println!("  Binaire       : {}", display_home(&exe));
    }
    if let Ok(backups) = backup::backups_dir() {
        println!("  Sauvegardes   : {}", display_home(&backups));
    }
    Ok(())
}

/// Affiche les commandes utiles après l'onboarding.
fn print_next_steps() {
    println!();
    println!("Prochaines étapes :");
    println!("  source ~/.bashrc");
    println!("  mnemo import");
    println!("  mnemo search");
    println!("  mnemo doctor");
}

/// Pose une question oui/non. En mode non interactif (atteint uniquement avec
/// `--yes`), retient la réponse par défaut.
fn ask(question: &str, default_yes: bool, interactive: bool) -> Result<bool> {
    if !interactive {
        return Ok(default_yes);
    }
    let suffix = if default_yes { "[O/n]" } else { "[o/N]" };
    print!("{question} {suffix} ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    if answer.is_empty() {
        return Ok(default_yes);
    }
    Ok(answer == "o" || answer == "oui" || answer == "y" || answer == "yes")
}

fn bashrc_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".bashrc"))
}

fn bash_history_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".bash_history"))
}

/// Raccourcit un chemin sous le répertoire personnel en `~/...` pour l'affichage.
fn display_home(path: &Path) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Ok(rest) = path.strip_prefix(&home) {
            return format!("~/{}", rest.display());
        }
    }
    path.display().to_string()
}
