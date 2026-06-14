//! Détection du « projet » courant et inventaire des projets connus.
//!
//! Stratégie de détection (du plus fiable au plus approximatif) :
//! 1. **Racine Git** (`git rev-parse --show-toplevel`) - priorité absolue ;
//! 2. **Fichier marqueur** (`package.json`, `Cargo.toml`, `pyproject.toml`,
//!    `go.mod`, `composer.json`) trouvé en remontant l'arborescence ;
//! 3. **Nom du dossier courant** en dernier recours.
//!
//! Cette logique n'altère jamais le champ historique `git_root` : elle ne sert
//! qu'à *résoudre* un nom de projet pour les filtres et l'affichage.

use anyhow::Result;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

use crate::db;
use crate::gitctx;

/// Origine de la détection d'un projet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectSource {
    /// Détecté via la racine d'un dépôt Git.
    Git,
    /// Détecté via un fichier marqueur d'écosystème.
    Marker(&'static str),
    /// Détecté via le simple nom du dossier courant.
    Directory,
}

impl ProjectSource {
    /// Libellé lisible de la source.
    pub fn label(&self) -> String {
        match self {
            ProjectSource::Git => "dépôt git".to_string(),
            ProjectSource::Marker(f) => format!("marqueur {f}"),
            ProjectSource::Directory => "dossier courant".to_string(),
        }
    }
}

/// Projet détecté pour un répertoire de travail.
#[derive(Debug, Clone)]
pub struct ProjectInfo {
    /// Nom court du projet (dernier segment de la racine).
    pub name: String,
    /// Racine du projet si connue (racine git ou dossier du marqueur).
    pub root: Option<PathBuf>,
    /// Comment le projet a été détecté.
    pub source: ProjectSource,
}

/// Fichiers marqueurs reconnus, par ordre de priorité.
const MARKERS: &[&str] = &[
    "Cargo.toml",
    "package.json",
    "pyproject.toml",
    "go.mod",
    "composer.json",
];

/// Détecte le projet associé à `cwd`. Ne renvoie jamais d'erreur : au pire on
/// retombe sur le nom du dossier courant.
pub fn detect(cwd: &Path) -> ProjectInfo {
    // 1. Racine Git prioritaire.
    let git = gitctx::detect(cwd);
    if let Some(root) = git.root {
        let path = PathBuf::from(&root);
        return ProjectInfo {
            name: base_name(&path).unwrap_or(root),
            root: Some(path),
            source: ProjectSource::Git,
        };
    }

    // 2. Fichier marqueur en remontant l'arborescence.
    if let Some((dir, marker)) = find_marker(cwd) {
        return ProjectInfo {
            name: base_name(&dir).unwrap_or_else(|| dir.display().to_string()),
            root: Some(dir),
            source: ProjectSource::Marker(marker),
        };
    }

    // 3. Nom du dossier courant.
    ProjectInfo {
        name: base_name(cwd).unwrap_or_else(|| "(inconnu)".to_string()),
        root: Some(cwd.to_path_buf()),
        source: ProjectSource::Directory,
    }
}

/// Nom du projet courant (résolution de `--project current`).
pub fn current_name() -> Option<String> {
    let cwd = std::env::current_dir().ok()?;
    Some(detect(&cwd).name)
}

/// Cherche le premier dossier marqueur en remontant depuis `start`.
fn find_marker(start: &Path) -> Option<(PathBuf, &'static str)> {
    let mut dir = Some(start);
    while let Some(d) = dir {
        for marker in MARKERS {
            if d.join(marker).is_file() {
                return Some((d.to_path_buf(), marker));
            }
        }
        dir = d.parent();
    }
    None
}

/// Dernier segment d'un chemin (nom de dossier), si présent.
fn base_name(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
}

/// Un projet présent dans l'historique (basé sur `git_root`).
#[derive(Debug, Clone)]
pub struct KnownProject {
    /// Racine git complète.
    pub root: String,
    /// Nom court (dernier segment).
    pub name: String,
    /// Nombre de commandes enregistrées pour ce projet.
    pub count: i64,
}

/// Liste les projets connus de l'historique, triés par nombre de commandes
/// décroissant. Seuls les `git_root` non nuls sont remontés.
pub fn list_known(conn: &Connection) -> Result<Vec<KnownProject>> {
    let mut stmt = conn.prepare(
        "SELECT git_root, COUNT(*) AS n
         FROM commands
         WHERE git_root IS NOT NULL AND git_root != ''
         GROUP BY git_root
         ORDER BY n DESC, git_root ASC",
    )?;
    let rows = stmt.query_map([], |row| {
        let root: String = row.get(0)?;
        let count: i64 = row.get(1)?;
        Ok((root, count))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (root, count) = row?;
        let name = base_name(Path::new(&root)).unwrap_or_else(|| root.clone());
        out.push(KnownProject { root, name, count });
    }
    Ok(out)
}

/// Affiche le projet courant (commande `mnemo project current`).
pub fn run_current() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let info = detect(&cwd);
    println!("Projet  : {}", info.name);
    if let Some(root) = &info.root {
        println!("Racine  : {}", root.display());
    }
    println!("Source  : {}", info.source.label());
    Ok(())
}

/// Affiche les projets connus de l'historique (commande `mnemo project list`).
pub fn run_list() -> Result<()> {
    let conn = db::open(&crate::config::db_path()?)?;
    let projects = list_known(&conn)?;
    if projects.is_empty() {
        println!("Aucun projet Git enregistré dans l'historique.");
        return Ok(());
    }
    println!("Projets connus ({}) :", projects.len());
    for p in &projects {
        println!("  {:<24} {:>6}  {}", p.name, p.count, p.root);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn detecte_un_marqueur_cargo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join("Cargo.toml"), "[package]\n").unwrap();
        let sub = root.join("src/inner");
        fs::create_dir_all(&sub).unwrap();

        let info = detect(&sub);
        assert!(matches!(info.source, ProjectSource::Marker("Cargo.toml")));
        assert_eq!(info.root.as_deref(), Some(root));
    }

    #[test]
    fn retombe_sur_le_nom_du_dossier() {
        let dir = tempfile::tempdir().unwrap();
        let sub = dir.path().join("projet-sans-marqueur");
        fs::create_dir_all(&sub).unwrap();

        let info = detect(&sub);
        assert_eq!(info.source, ProjectSource::Directory);
        assert_eq!(info.name, "projet-sans-marqueur");
    }

    #[test]
    fn list_known_agrege_par_racine() {
        let conn = db::open_in_memory().unwrap();
        for (cmd, root) in [
            ("a", "/home/u/proj-a"),
            ("b", "/home/u/proj-a"),
            ("c", "/home/u/proj-b"),
        ] {
            db::insert_command(
                &conn,
                &db::NewCommand {
                    command: cmd.into(),
                    cwd: Some(root.into()),
                    shell: Some("bash".into()),
                    hostname: Some("h".into()),
                    exit_code: Some(0),
                    created_at: "2026-06-14 10:00:00".into(),
                    git_root: Some(root.into()),
                    git_branch: None,
                    git_remote: None,
                    session_id: None,
                },
            )
            .unwrap();
        }
        let projects = list_known(&conn).unwrap();
        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0].name, "proj-a");
        assert_eq!(projects[0].count, 2);
    }
}
