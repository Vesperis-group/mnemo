//! Helpers de rendu Markdown et d'affichage partagés.
//!
//! Ces fonctions encodent du texte arbitraire (commandes shell, chemins) sans
//! jamais casser la structure d'un document Markdown : elles échappent les
//! pipes des tableaux, neutralisent les retours à la ligne et choisissent des
//! clôtures de code plus longues que toute suite de backticks interne. Elles
//! sont réutilisées par `mnemo session` et `mnemo project` afin de garantir un
//! rendu cohérent et robuste.

/// Plus longue suite consécutive de backticks dans `s`.
pub fn longest_backtick_run(s: &str) -> usize {
    let mut max = 0;
    let mut cur = 0;
    for ch in s.chars() {
        if ch == '`' {
            cur += 1;
            max = max.max(cur);
        } else {
            cur = 0;
        }
    }
    max
}

/// Encadre `commands` dans un bloc de code Markdown, en choisissant une clôture
/// plus longue que toute suite de backticks présente, pour ne jamais casser le
/// bloc.
pub fn md_code_block(commands: &[String]) -> String {
    let max_run = commands
        .iter()
        .map(|c| longest_backtick_run(c))
        .max()
        .unwrap_or(0);
    let fence = "`".repeat(max_run.max(2) + 1);
    let mut out = String::new();
    out.push_str(&fence);
    out.push_str("bash\n");
    for c in commands {
        out.push_str(c);
        out.push('\n');
    }
    out.push_str(&fence);
    out.push('\n');
    out
}

/// Rend une chaîne en code en ligne Markdown, robuste aux backticks internes.
pub fn md_inline_code(s: &str) -> String {
    let ticks = "`".repeat(longest_backtick_run(s) + 1);
    let pad = if s.starts_with('`') || s.ends_with('`') {
        " "
    } else {
        ""
    };
    format!("{ticks}{pad}{s}{pad}{ticks}")
}

/// Échappe une cellule de tableau Markdown en texte simple (pipes et retours).
pub fn md_table_cell_text(s: &str) -> String {
    s.replace('|', "\\|").replace(['\n', '\r'], " ")
}

/// Rend une commande dans une cellule de tableau Markdown, en code en ligne,
/// sans casser la structure du tableau.
pub fn md_table_cell_code(s: &str) -> String {
    let oneline = s.replace(['\n', '\r'], " ");
    let escaped = oneline.replace('|', "\\|");
    md_inline_code(&escaped)
}

/// Raccourcit un chemin sous le répertoire personnel en `~/...`.
pub fn display_home(path: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        if let Some(home_str) = home.to_str() {
            if let Some(rest) = path.strip_prefix(home_str) {
                let rest = rest.trim_start_matches('/');
                if rest.is_empty() {
                    return "~".to_string();
                }
                return format!("~/{rest}");
            }
        }
    }
    path.to_string()
}

/// Affiche une option de chemin avec raccourci `~`, ou `-` si absente.
pub fn opt_home(value: &Option<String>) -> String {
    value
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(display_home)
        .unwrap_or_else(|| "-".to_string())
}

/// Affiche une option textuelle, ou `-` si absente.
pub fn opt(value: &Option<String>) -> String {
    value
        .as_deref()
        .filter(|s| !s.is_empty())
        .unwrap_or("-")
        .to_string()
}

/// Partie horaire (`HH:MM:SS`) d'un horodatage `YYYY-MM-DD HH:MM:SS`.
pub fn time_part(created_at: &str) -> &str {
    created_at.split_whitespace().nth(1).unwrap_or(created_at)
}

/// Tronque un horodatage à la minute (`YYYY-MM-DD HH:MM`).
pub fn short_datetime(created_at: &str) -> &str {
    created_at.get(..16).unwrap_or(created_at)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_code_protege_les_backticks() {
        assert_eq!(md_inline_code("ls"), "`ls`");
        assert_eq!(md_inline_code("echo `date`"), "`` echo `date` ``");
    }

    #[test]
    fn code_block_choisit_une_cloture_assez_longue() {
        let block = md_code_block(&["echo ```x```".to_string()]);
        assert!(block.starts_with("````bash\n"));
        assert!(block.trim_end().ends_with("````"));
    }

    #[test]
    fn table_cell_echappe_les_pipes() {
        let cell = md_table_cell_code("grep -E 'a|b'");
        assert!(cell.contains("\\|"));
        assert!(!cell.contains("a|b"));
    }

    #[test]
    fn horaire_et_date_courte() {
        assert_eq!(time_part("2026-06-23 10:12:01"), "10:12:01");
        assert_eq!(short_datetime("2026-06-23 10:12:01"), "2026-06-23 10:12");
    }
}
