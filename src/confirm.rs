//! Confirmation interactive pour les opérations destructives.
//!
//! Règles de sécurité (v0.3) :
//! - Aucune opération destructive ne s'exécute sans accord explicite.
//! - L'accord vient soit de l'option `--yes`, soit d'une réponse interactive.
//! - **En mode non interactif** (stdin n'est pas un terminal), une opération
//!   destructive sans `--yes` est refusée : on ne supprime jamais « en
//!   silence » depuis un script ou un pipe.

use anyhow::Result;
use std::io::{self, IsTerminal, Write};

/// Décide si une opération destructive peut se poursuivre.
///
/// - `assume_yes` (option `--yes`) : autorise directement.
/// - Sinon, si stdin est un terminal : pose la question `prompt` (réponse
///   `o`/`y` pour accepter, tout le reste refuse).
/// - Sinon (non interactif, pas de `--yes`) : refuse et l'explique.
pub fn confirm(prompt: &str, assume_yes: bool) -> Result<bool> {
    if assume_yes {
        return Ok(true);
    }

    if !io::stdin().is_terminal() {
        eprintln!(
            "Opération annulée : entrée non interactive. Relancez avec --yes pour confirmer."
        );
        return Ok(false);
    }

    print!("{prompt} [o/N] ");
    io::stdout().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    let answer = answer.trim().to_lowercase();
    Ok(answer == "o" || answer == "oui" || answer == "y" || answer == "yes")
}
