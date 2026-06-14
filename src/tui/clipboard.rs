//! Copie vers le presse-papiers système sans dépendance obligatoire.
//!
//! On tente, dans l'ordre, les utilitaires Linux disponibles (`wl-copy`,
//! `xclip`, `xsel`). Si aucun n'est présent (WSL minimal, headless…), la copie
//! échoue proprement et l'appelant retombe sur l'impression via `Entrée`.
//! Aucune bibliothèque de presse-papiers graphique n'est liée au binaire.

use std::io::Write;
use std::process::{Command, Stdio};

/// Tente de copier `text` vers le presse-papiers système.
///
/// Renvoie `Ok(true)` si un utilitaire a accepté le texte, `Ok(false)` si aucun
/// n'est disponible.
pub fn copy_to_clipboard(text: &str) -> std::io::Result<bool> {
    const CANDIDATES: &[(&str, &[&str])] = &[
        ("wl-copy", &[]),
        ("xclip", &["-selection", "clipboard"]),
        ("xsel", &["--clipboard", "--input"]),
    ];

    for (program, args) in CANDIDATES {
        match try_copy(program, args, text) {
            Ok(true) => return Ok(true),
            // Programme absent ou en échec : on essaie le suivant.
            _ => continue,
        }
    }
    Ok(false)
}

/// Lance `program args`, écrit `text` sur son stdin et renvoie `Ok(true)` si le
/// processus se termine avec succès.
fn try_copy(program: &str, args: &[&str], text: &str) -> std::io::Result<bool> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(text.as_bytes())?;
        // `stdin` est libéré ici (fermeture du tube), ce qui débloque l'outil.
    }

    let status = child.wait()?;
    Ok(status.success())
}
