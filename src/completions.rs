//! Commande `mnemo completions <shell>` : génère un script de complétion shell
//! sur stdout (bash, zsh, fish).
//!
//! mnemo n'écrit jamais dans les fichiers shell de l'utilisateur ni ne modifie
//! `.bashrc`, `.zshrc` ou la configuration Fish : la sortie est produite sur
//! stdout et l'utilisateur la redirige lui-même vers l'emplacement adéquat.

use anyhow::Result;
use clap::CommandFactory;
use clap_complete::generate;
use std::io;

use crate::cli::{Cli, CompletionShell};

/// Écrit le script de complétion du shell demandé sur stdout.
pub fn run(shell: CompletionShell) -> Result<()> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_string();
    generate(shell.generator(), &mut cmd, bin_name, &mut io::stdout());
    Ok(())
}
