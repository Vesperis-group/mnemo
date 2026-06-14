//! Helpers de formatage purs pour la TUI (sans dépendance au terminal).
//!
//! Ces fonctions sont isolées ici pour être testées unitairement : troncature
//! des chemins et commandes, extraction de l'heure, libellé de contexte. Elles
//! ne paniquent jamais, y compris sur des entrées vides, très longues ou
//! multi-octets (UTF-8).

use crate::db::CommandRecord;
use crate::tui::app::last_segment;

/// Caractère d'ellipse utilisé pour les troncatures.
const ELLIPSIS: char = '…';

/// Extrait l'heure `HH:MM` d'un horodatage `YYYY-MM-DD HH:MM:SS`.
///
/// Retombe sur la chaîne complète si le format est inattendu (jamais de panique).
pub fn short_time(created_at: &str) -> &str {
    // Position 11..16 correspond à `HH:MM` dans `YYYY-MM-DD HH:MM:SS`.
    created_at.get(11..16).unwrap_or(created_at)
}

/// Tronque `s` à `max` caractères en ajoutant une ellipse finale si nécessaire.
///
/// `max == 0` renvoie une chaîne vide. La troncature se fait sur les caractères
/// (pas les octets) pour rester correcte en UTF-8.
pub fn truncate_end(s: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max == 1 {
        return ELLIPSIS.to_string();
    }
    let mut out: String = s.chars().take(max - 1).collect();
    out.push(ELLIPSIS);
    out
}

/// Tronque `s` en conservant le début et la fin, avec une ellipse au milieu.
///
/// Utile pour les chemins (`/home/.../mnemo/backups`). `max == 0` renvoie une
/// chaîne vide ; en deçà de 5 caractères on retombe sur une troncature de fin.
pub fn truncate_middle(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    if max < 5 {
        return truncate_end(s, max);
    }
    // Réserve un caractère pour l'ellipse, répartit le reste tête/queue.
    let keep = max - 1;
    let head = keep.div_ceil(2);
    let tail = keep - head;
    let chars: Vec<char> = s.chars().collect();
    let mut out: String = chars[..head].iter().collect();
    out.push(ELLIPSIS);
    out.extend(chars[count - tail..].iter());
    out
}

/// Libellé de contexte d'une commande : nom du projet Git si disponible, sinon
/// dernier segment du répertoire de travail, sinon `-`.
pub fn context_label(record: &CommandRecord) -> String {
    if let Some(root) = &record.git_root {
        if !root.is_empty() {
            return last_segment(root).to_string();
        }
    }
    if let Some(cwd) = &record.cwd {
        if !cwd.is_empty() {
            return last_segment(cwd).to_string();
        }
    }
    "-".to_string()
}

/// Statut court (`SUCCESS` / `FAILED` / `-`) à partir du code de sortie.
pub fn status_text(exit_code: Option<i64>) -> &'static str {
    match exit_code {
        Some(0) => "SUCCESS",
        Some(_) => "FAILED",
        None => "-",
    }
}

/// Symbole de statut (`✓` / `✗` / ` `) à partir du code de sortie.
pub fn status_symbol(exit_code: Option<i64>) -> &'static str {
    match exit_code {
        Some(0) => "✓",
        Some(_) => "✗",
        None => "·",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(command: &str) -> CommandRecord {
        CommandRecord {
            id: 1,
            command: command.to_string(),
            cwd: None,
            shell: None,
            hostname: None,
            exit_code: None,
            created_at: "2026-06-14 17:29:11".to_string(),
            git_root: None,
            git_branch: None,
            git_remote: None,
            session_id: None,
        }
    }

    #[test]
    fn short_time_extrait_hh_mm() {
        assert_eq!(short_time("2026-06-14 17:29:11"), "17:29");
    }

    #[test]
    fn short_time_tolere_format_inattendu() {
        assert_eq!(short_time("court"), "court");
        assert_eq!(short_time(""), "");
    }

    #[test]
    fn truncate_end_court_inchange() {
        assert_eq!(truncate_end("git push", 20), "git push");
    }

    #[test]
    fn truncate_end_ajoute_ellipse() {
        let out = truncate_end("git push origin main", 8);
        assert_eq!(out.chars().count(), 8);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn truncate_end_cas_limites() {
        assert_eq!(truncate_end("abc", 0), "");
        assert_eq!(truncate_end("abc", 1), "…");
    }

    #[test]
    fn truncate_end_ne_panique_pas_sur_utf8() {
        let s = "éàçü emojis 🚀🚀🚀 accentués";
        let out = truncate_end(s, 5);
        assert_eq!(out.chars().count(), 5);
    }

    #[test]
    fn truncate_middle_conserve_tete_et_queue() {
        let path = "/home/killian/projects/mnemo/backups";
        let out = truncate_middle(path, 20);
        assert_eq!(out.chars().count(), 20);
        assert!(out.contains('…'));
        assert!(out.starts_with('/'));
        assert!(out.ends_with('s'));
    }

    #[test]
    fn truncate_middle_court_inchange() {
        assert_eq!(truncate_middle("/tmp", 20), "/tmp");
    }

    #[test]
    fn context_label_prefere_projet_git() {
        let mut r = rec("cargo build");
        r.git_root = Some("/home/killian/mnemo".to_string());
        assert_eq!(context_label(&r), "mnemo");
    }

    #[test]
    fn context_label_retombe_sur_dossier() {
        let mut r = rec("ls");
        r.cwd = Some("/var/log".to_string());
        assert_eq!(context_label(&r), "log");
    }

    #[test]
    fn context_label_sans_contexte() {
        assert_eq!(context_label(&rec("ls")), "-");
    }

    #[test]
    fn status_text_et_symbole() {
        assert_eq!(status_text(Some(0)), "SUCCESS");
        assert_eq!(status_text(Some(1)), "FAILED");
        assert_eq!(status_text(None), "-");
        assert_eq!(status_symbol(Some(0)), "✓");
        assert_eq!(status_symbol(Some(2)), "✗");
    }
}
