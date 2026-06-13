/// Snippet d'intégration Bash à ajouter dans `~/.bashrc`.
///
/// - `__mnemo_record` est branché sur `PROMPT_COMMAND` et enregistre la
///   dernière commande exécutée (avec son code de sortie et le répertoire).
/// - La commande `mnemo` elle-même n'est jamais enregistrée.
/// - `Ctrl+R` est remappé pour ouvrir la recherche TUI de mnemo.
pub fn bashrc_snippet() -> String {
    SNIPPET.to_string()
}

/// Marqueurs externes encadrant le bloc ajouté par l'installateur et
/// `mnemo doctor --fix` (identiques à ceux de `scripts/lib/bashrc.sh`).
pub const BLOCK_BEGIN: &str = "# >>> mnemo init >>>";
pub const BLOCK_END: &str = "# <<< mnemo init <<<";

/// Marqueur interne du snippet lui-même, présent dans tout bloc mnemo.
const SNIPPET_BEGIN: &str = "# >>> mnemo >>>";

/// Bloc complet à écrire dans le `.bashrc` (marqueurs externes + snippet).
pub fn wrapped_block() -> String {
    format!("{BLOCK_BEGIN}\n{}{BLOCK_END}\n", bashrc_snippet())
}

/// Indique si un contenu de `.bashrc` contient déjà l'intégration mnemo.
pub fn has_block(content: &str) -> bool {
    content.contains(SNIPPET_BEGIN) || content.contains("__mnemo_record")
}

/// Nombre de blocs mnemo détectés (sert à repérer les doublons).
pub fn count_blocks(content: &str) -> usize {
    content.matches(SNIPPET_BEGIN).count()
}

/// Indique si le bind `Ctrl+R` de mnemo est présent.
pub fn has_ctrl_r_bind(content: &str) -> bool {
    content.contains("__mnemo_search") || content.contains("\\C-r")
}

/// Horodatage compact `YYYYMMDD-HHMMSS` pour les noms de sauvegarde.
fn compact_now() -> String {
    let ts = crate::db::now_timestamp(); // "YYYY-MM-DD HH:MM:SS"
    let date = ts.get(0..10).unwrap_or("").replace('-', "");
    let time = ts.get(11..19).unwrap_or("").replace(':', "");
    format!("{date}-{time}")
}

/// Ajoute le bloc mnemo au `.bashrc` s'il est absent, après sauvegarde.
///
/// - Idempotent : ne fait rien (et ne crée pas de sauvegarde) si le bloc est
///   déjà présent.
/// - Crée une sauvegarde `<bashrc>.mnemo.bak.YYYYMMDD-HHMMSS` si le fichier
///   existe déjà.
///
/// Retourne `Ok(true)` si le bloc a été ajouté, `Ok(false)` s'il existait déjà.
pub fn install_block(bashrc: &std::path::Path) -> anyhow::Result<bool> {
    let existing = std::fs::read_to_string(bashrc).unwrap_or_default();
    if has_block(&existing) {
        return Ok(false);
    }

    if bashrc.exists() {
        let backup = bashrc.with_file_name(format!(".bashrc.mnemo.bak.{}", compact_now()));
        std::fs::copy(bashrc, &backup)?;
    }

    let mut content = existing;
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&wrapped_block());
    std::fs::write(bashrc, content)?;
    Ok(true)
}

const SNIPPET: &str = r#"# >>> mnemo >>>
# Enregistre automatiquement chaque commande dans mnemo.
__mnemo_record() {
    local __mnemo_exit=$?
    local __mnemo_cmd
    __mnemo_cmd=$(HISTTIMEFORMAT='' history 1 2>/dev/null | sed 's/^ *[0-9]\+ *//')
    if [ -n "$__mnemo_cmd" ] && [ "$__mnemo_cmd" != "$__MNEMO_LAST_CMD" ]; then
        case "$__mnemo_cmd" in
            mnemo|mnemo\ *) ;;
            *)
                __MNEMO_LAST_CMD="$__mnemo_cmd"
                mnemo add --cmd "$__mnemo_cmd" --cwd "$PWD" --exit-code "$__mnemo_exit" >/dev/null 2>&1
                ;;
        esac
    fi
    return $__mnemo_exit
}
case "$PROMPT_COMMAND" in
    *__mnemo_record*) ;;
    *) PROMPT_COMMAND="__mnemo_record${PROMPT_COMMAND:+; $PROMPT_COMMAND}" ;;
esac

# Ctrl+R : ouvre la recherche TUI et insère la commande choisie.
__mnemo_search() {
    local __mnemo_selected
    __mnemo_selected=$(mnemo search 2>/dev/null)
    if [ -n "$__mnemo_selected" ]; then
        READLINE_LINE="$__mnemo_selected"
        READLINE_POINT=${#READLINE_LINE}
    fi
}
bind -x '"\C-r": __mnemo_search' 2>/dev/null
# <<< mnemo <<<
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snippet_contient_les_elements_cles() {
        let s = bashrc_snippet();
        assert!(s.contains("__mnemo_record"));
        assert!(s.contains("PROMPT_COMMAND"));
        assert!(s.contains("mnemo add"));
        // La commande mnemo elle-même doit être exclue.
        assert!(s.contains("mnemo|mnemo\\ *"));
    }

    #[test]
    fn detection_du_bloc_et_des_doublons() {
        let empty = "export FOO=1\n";
        assert!(!has_block(empty));
        assert_eq!(count_blocks(empty), 0);

        let one = wrapped_block();
        assert!(has_block(&one));
        assert_eq!(count_blocks(&one), 1);
        assert!(has_ctrl_r_bind(&one));

        let two = format!("{one}\n{one}");
        assert_eq!(count_blocks(&two), 2);
    }
}
