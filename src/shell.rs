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

/// Résultat d'une réparation du bloc `.bashrc` par `mnemo doctor --fix`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockRepair {
    /// Bloc ajouté (il était absent).
    Created,
    /// Doublons supprimés, un seul bloc propre conservé.
    Deduplicated,
    /// Bloc régénéré pour restaurer le raccourci `Ctrl+R`.
    CtrlRRestored,
    /// Rien à faire : un unique bloc complet était déjà présent.
    AlreadyOk,
}

/// Retire tous les blocs mnemo encadrés par les marqueurs externes.
///
/// Fonction pure : ne touche pas au disque. Les lignes situées entre
/// [`BLOCK_BEGIN`] et [`BLOCK_END`] (inclus) sont supprimées.
pub fn strip_blocks(content: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == BLOCK_BEGIN {
            in_block = true;
            continue;
        }
        if trimmed == BLOCK_END {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Répare le bloc mnemo du `.bashrc` : ajoute s'il manque, déduplique s'il est
/// présent plusieurs fois, ou régénère un bloc complet si `Ctrl+R` a disparu.
///
/// Toujours précédé d'une sauvegarde `<bashrc>.mnemo.bak.YYYYMMDD-HHMMSS` avant
/// toute modification. Non destructif vis-à-vis du reste du fichier.
pub fn repair_block(bashrc: &std::path::Path) -> anyhow::Result<BlockRepair> {
    let existing = std::fs::read_to_string(bashrc).unwrap_or_default();

    if !has_block(&existing) {
        install_block(bashrc)?;
        return Ok(BlockRepair::Created);
    }

    let duplicated = count_blocks(&existing) > 1;
    let missing_ctrl_r = !has_ctrl_r_bind(&existing);
    if !duplicated && !missing_ctrl_r {
        return Ok(BlockRepair::AlreadyOk);
    }

    if bashrc.exists() {
        let backup = bashrc.with_file_name(format!(".bashrc.mnemo.bak.{}", compact_now()));
        std::fs::copy(bashrc, &backup)?;
    }

    let mut content = strip_blocks(&existing);
    while content.ends_with("\n\n") {
        content.pop();
    }
    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&wrapped_block());
    std::fs::write(bashrc, content)?;

    Ok(if duplicated {
        BlockRepair::Deduplicated
    } else {
        BlockRepair::CtrlRRestored
    })
}

const SNIPPET: &str = r#"# >>> mnemo >>>
# Identifiant de session : regroupe les commandes d'un même shell interactif
# (voir `mnemo session`). Conservé pour toute la durée de vie du shell.
if [ -z "${MNEMO_SESSION_ID:-}" ]; then
    export MNEMO_SESSION_ID="$(date +%Y%m%dT%H%M%S)-$$"
fi
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

    #[test]
    fn strip_blocks_retire_le_bloc_encadre() {
        let avant = format!("export FOO=1\n{}export BAR=2\n", wrapped_block());
        let apres = strip_blocks(&avant);
        assert!(!has_block(&apres));
        assert!(apres.contains("export FOO=1"));
        assert!(apres.contains("export BAR=2"));
    }

    #[test]
    fn repair_block_deduplique_et_ajoute() {
        let dir = tempfile::tempdir().unwrap();
        let bashrc = dir.path().join(".bashrc");

        // Absent -> créé.
        std::fs::write(&bashrc, "export FOO=1\n").unwrap();
        assert_eq!(repair_block(&bashrc).unwrap(), BlockRepair::Created);
        assert_eq!(count_blocks(&std::fs::read_to_string(&bashrc).unwrap()), 1);

        // Déjà correct -> aucune action.
        assert_eq!(repair_block(&bashrc).unwrap(), BlockRepair::AlreadyOk);

        // Doublon -> dédupliqué.
        let one = std::fs::read_to_string(&bashrc).unwrap();
        std::fs::write(&bashrc, format!("{one}{}", wrapped_block())).unwrap();
        assert_eq!(repair_block(&bashrc).unwrap(), BlockRepair::Deduplicated);
        let after = std::fs::read_to_string(&bashrc).unwrap();
        assert_eq!(count_blocks(&after), 1);
        assert!(has_ctrl_r_bind(&after));
    }
}
