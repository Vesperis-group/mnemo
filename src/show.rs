//! Consultation détaillée (`mnemo show`) et récupération brute (`mnemo print`)
//! d'une commande de l'historique, par identifiant.
//!
//! Ces deux sous-commandes sont en lecture seule : elles lisent la base et
//! n'exécutent **jamais** la commande stockée. Si une commande a déjà été
//! redactée (`mnemo secrets redact`), c'est sa forme redactée enregistrée qui
//! est affichée, sans nouvelle analyse de secrets.

use anyhow::{bail, Result};
use std::io::Write;

use crate::config;
use crate::db::{self, CommandRecord};

/// `mnemo show <id>` : affiche le détail complet d'une commande.
pub fn run_show(id: i64) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;
    match db::get_command(&conn, id)? {
        Some(record) => {
            // stdout verrouillé : un `BrokenPipe` (sortie pipée) remonte comme
            // une erreur propre plutôt que de faire paniquer `print!`.
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            write!(out, "{}", format_record(&record))?;
            Ok(())
        }
        None => bail!("Aucune commande avec l'ID {id}."),
    }
}

/// `mnemo print <id>` : imprime uniquement la commande brute sur stdout.
///
/// La sortie contient exactement la commande suivie d'un saut de ligne, sans
/// label ni couleur. En cas d'ID inexistant, l'erreur part sur stderr et le
/// code de sortie est non nul (via la remontée d'erreur de `main`).
pub fn run_print(id: i64) -> Result<()> {
    let conn = db::open(&config::db_path()?)?;
    match db::get_command(&conn, id)? {
        Some(record) => {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            writeln!(out, "{}", record.command)?;
            Ok(())
        }
        None => bail!("Aucune commande avec l'ID {id}."),
    }
}

/// Met en forme l'affichage détaillé de `mnemo show`.
///
/// Les champs absents (ou vides) sont **omis** plutôt qu'inventés ou remplis
/// d'une valeur factice. Le bloc commande reproduit fidèlement la valeur
/// stockée (éventuellement déjà redactée).
fn format_record(r: &CommandRecord) -> String {
    let mut out = String::new();
    out.push_str(&format!("Commande {}\n\n", r.id));
    out.push_str(&format!("Date : {}\n", r.created_at));
    if let Some(cwd) = field(&r.cwd) {
        out.push_str(&format!("Dossier : {cwd}\n"));
    }
    if let Some(root) = field(&r.git_root) {
        out.push_str(&format!("Projet Git : {root}\n"));
    }
    if let Some(branch) = field(&r.git_branch) {
        out.push_str(&format!("Branche : {branch}\n"));
    }
    if let Some(remote) = field(&r.git_remote) {
        out.push_str(&format!("Remote : {remote}\n"));
    }
    if let Some(shell) = field(&r.shell) {
        out.push_str(&format!("Shell : {shell}\n"));
    }
    if let Some(session) = field(&r.session_id) {
        out.push_str(&format!("Session : {session}\n"));
    }
    if let Some(code) = r.exit_code {
        out.push_str(&format!("Code retour : {code}\n"));
    }
    out.push_str(&format!("\nCommande :\n{}\n", r.command));
    out
}

/// Renvoie la valeur si elle est présente et non vide (après trim), sinon
/// `None`. Évite d'afficher des lignes vides pour des champs non renseignés.
fn field(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|s| !s.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec() -> CommandRecord {
        CommandRecord {
            id: 128,
            command: "cargo test --locked".to_string(),
            cwd: Some("/home/u/projects/mnemo".to_string()),
            shell: Some("bash".to_string()),
            hostname: None,
            exit_code: Some(0),
            created_at: "2026-06-23 14:22:10".to_string(),
            git_root: Some("/home/u/projects/mnemo".to_string()),
            git_branch: Some("main".to_string()),
            git_remote: None,
            session_id: Some("20260623T141200-12345".to_string()),
        }
    }

    #[test]
    fn format_affiche_les_champs_disponibles() {
        let out = format_record(&rec());
        assert!(out.contains("Commande 128"));
        assert!(out.contains("Date : 2026-06-23 14:22:10"));
        assert!(out.contains("Dossier : /home/u/projects/mnemo"));
        assert!(out.contains("Projet Git : /home/u/projects/mnemo"));
        assert!(out.contains("Branche : main"));
        assert!(out.contains("Session : 20260623T141200-12345"));
        assert!(out.contains("Code retour : 0"));
        assert!(out.contains("Commande :\ncargo test --locked\n"));
    }

    #[test]
    fn format_omet_les_champs_absents() {
        let mut r = rec();
        r.git_remote = None;
        r.git_branch = None;
        r.session_id = None;
        r.git_root = Some("   ".to_string()); // vide après trim -> omis
        let out = format_record(&r);
        assert!(!out.contains("Branche :"));
        assert!(!out.contains("Remote :"));
        assert!(!out.contains("Session :"));
        assert!(!out.contains("Projet Git :"));
    }

    #[test]
    fn format_preserve_une_commande_redactee() {
        let mut r = rec();
        r.command = "curl -H 'Authorization: Bearer [REDACTED]'".to_string();
        let out = format_record(&r);
        assert!(out.contains("[REDACTED]"));
    }
}
