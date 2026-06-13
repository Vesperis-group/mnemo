use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Mots-clés sensibles ignorés par défaut lors de l'import / de l'ajout.
const DEFAULT_SENSITIVE: &[&str] = &[
    "password",
    "passwd",
    "token",
    "secret",
    "api_key",
    "bearer",
    "private_key",
    "sshpass",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Commandes contenant un de ces mots-clés sont ignorées.
    pub sensitive_keywords: Vec<String>,
    /// Préfixes de commandes à ne jamais enregistrer (ex: "mnemo").
    pub ignore_prefixes: Vec<String>,
    /// Nombre maximal de commandes chargées dans la TUI.
    pub search_limit: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sensitive_keywords: DEFAULT_SENSITIVE.iter().map(|s| s.to_string()).collect(),
            ignore_prefixes: vec!["mnemo".to_string()],
            search_limit: 5000,
        }
    }
}

impl Config {
    /// Charge la config depuis `~/.config/mnemo/config.toml`, ou les valeurs
    /// par défaut si le fichier n'existe pas encore.
    pub fn load() -> Result<Self> {
        let path = config_path()?;
        if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("lecture de la config {}", path.display()))?;
            let cfg: Config = toml::from_str(&raw)
                .with_context(|| format!("parsing TOML de {}", path.display()))?;
            Ok(cfg)
        } else {
            Ok(Config::default())
        }
    }

    /// Écrit la config au format TOML, en créant le dossier parent si besoin.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("création du dossier {}", parent.display()))?;
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw)
            .with_context(|| format!("écriture de la config {}", path.display()))?;
        Ok(())
    }
}

pub fn config_dir() -> Result<PathBuf> {
    let base = dirs::config_dir().context("dossier de configuration introuvable")?;
    Ok(base.join("mnemo"))
}

pub fn config_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("config.toml"))
}

pub fn data_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().context("dossier de données introuvable")?;
    Ok(base.join("mnemo"))
}

pub fn db_path() -> Result<PathBuf> {
    Ok(data_dir()?.join("history.db"))
}
