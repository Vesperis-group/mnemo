//! Détection du contexte Git d'un répertoire, pour enrichir les commandes.
//!
//! Git est **optionnel** : si l'exécutable est absent, si le répertoire n'est
//! pas un dépôt, ou si une commande échoue/dépasse le délai imparti, les champs
//! concernés valent simplement `None`. mnemo ne dépend jamais de Git pour
//! fonctionner.

use std::io::Read;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Délai maximal accordé à une commande `git` avant abandon (fallback propre).
const GIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Contexte Git associé à un répertoire de travail.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GitContext {
    /// Racine du dépôt (`git rev-parse --show-toplevel`).
    pub root: Option<String>,
    /// Branche courante (`git branch --show-current`).
    pub branch: Option<String>,
    /// URL du remote `origin` (`git config --get remote.origin.url`).
    pub remote: Option<String>,
}

impl GitContext {
    /// Vrai si aucune information Git n'a pu être collectée.
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.root.is_none() && self.branch.is_none() && self.remote.is_none()
    }
}

/// Détecte le contexte Git pour `cwd`. Ne renvoie jamais d'erreur : en cas de
/// problème (pas de Git, hors dépôt, timeout…), les champs sont à `None`.
pub fn detect(cwd: &Path) -> GitContext {
    // On commence par la racine : si ce n'est pas un dépôt, inutile d'aller plus
    // loin (les autres appels seraient de toute façon vides).
    let root = run_git(cwd, &["rev-parse", "--show-toplevel"]);
    if root.is_none() {
        return GitContext::default();
    }
    GitContext {
        root,
        branch: run_git(cwd, &["branch", "--show-current"]),
        remote: run_git(cwd, &["config", "--get", "remote.origin.url"]),
    }
}

/// Exécute `git -C <cwd> <args...>` avec un délai maximal. Renvoie la sortie
/// standard nettoyée si la commande réussit et produit un résultat non vide,
/// sinon `None`.
fn run_git(cwd: &Path, args: &[&str]) -> Option<String> {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        // Évite tout blocage interactif (prompt d'identifiants, pager, verrous).
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;

    let deadline = Instant::now() + GIT_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return None;
                }
                let mut out = String::new();
                child.stdout.take()?.read_to_string(&mut out).ok()?;
                let trimmed = out.trim();
                return if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed.to_string())
                };
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn hors_depot_renvoie_un_contexte_vide() {
        // /tmp n'est (normalement) pas un dépôt Git.
        let ctx = detect(&PathBuf::from("/"));
        assert!(ctx.is_empty(), "racine systeme ne doit pas etre un depot");
    }

    #[test]
    fn chemin_inexistant_ne_panique_pas() {
        let ctx = detect(&PathBuf::from("/chemin/qui/n/existe/pas/12345"));
        assert!(ctx.is_empty());
    }
}
