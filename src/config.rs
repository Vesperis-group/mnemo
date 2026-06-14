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
    /// Options propres à `mnemo stats`.
    pub stats: StatsConfig,
    /// Options de maintenance / nettoyage automatique.
    pub maintenance: MaintenanceConfig,
}

/// Configuration de la commande `mnemo stats`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct StatsConfig {
    /// Noms de commandes (normalisés) à exclure du « Top commandes ». Les
    /// commandes concernées restent en base ; elles sont seulement comptées
    /// dans « Entrées ignorées ».
    pub ignored_commands: Vec<String>,
}

/// Configuration du nettoyage automatique (`mnemo maintenance`).
///
/// Désactivé par défaut : mnemo ne supprime jamais de données sans action
/// explicite de l'utilisateur (`--yes`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MaintenanceConfig {
    /// Active la possibilité de nettoyage automatique par ancienneté.
    pub auto_prune_enabled: bool,
    /// Ancienneté au-delà de laquelle les commandes sont éligibles au nettoyage
    /// (durée lisible : `180d`, `26w`, `6m`, `1y`).
    pub auto_prune_after: String,
    /// Crée systématiquement une sauvegarde avant tout nettoyage automatique.
    pub auto_backup_before_prune: bool,
}

impl Default for MaintenanceConfig {
    fn default() -> Self {
        Self {
            auto_prune_enabled: false,
            auto_prune_after: "180d".to_string(),
            auto_backup_before_prune: true,
        }
    }
}

/// Gravité d'un problème de configuration relevé par [`Config::validate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IssueLevel {
    /// Bloquant : la valeur est inutilisable en l'état.
    Error,
    /// Non bloquant : la valeur est inhabituelle mais tolérée.
    Warning,
}

/// Problème relevé lors de la validation de la configuration.
#[derive(Debug, Clone)]
pub struct ConfigIssue {
    pub level: IssueLevel,
    pub message: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sensitive_keywords: DEFAULT_SENSITIVE.iter().map(|s| s.to_string()).collect(),
            ignore_prefixes: vec!["mnemo".to_string()],
            search_limit: 5000,
            stats: StatsConfig::default(),
            maintenance: MaintenanceConfig::default(),
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
            harden_dir(parent);
        }
        let raw = toml::to_string_pretty(self)?;
        std::fs::write(path, raw)
            .with_context(|| format!("écriture de la config {}", path.display()))?;
        // La configuration contient des réglages locaux : permissions privées.
        harden_file(path);
        Ok(())
    }

    /// Normalise un nom de commande pour la liste d'exclusion des stats :
    /// trim + minuscules. Stratégie simple et documentée (comparaison exacte,
    /// insensible à la casse).
    pub fn normalize_ignored(name: &str) -> String {
        name.trim().to_lowercase()
    }

    /// Ajoute une commande à `stats.ignored_commands` si absente.
    /// Retourne `true` si la liste a changé, `false` si déjà présente.
    pub fn add_ignored_command(&mut self, name: &str) -> bool {
        let normalized = Self::normalize_ignored(name);
        if self.stats.ignored_commands.contains(&normalized) {
            return false;
        }
        self.stats.ignored_commands.push(normalized);
        self.stats.ignored_commands.sort();
        true
    }

    /// Retire une commande de `stats.ignored_commands` si présente.
    /// Retourne `true` si la liste a changé, `false` si absente.
    pub fn remove_ignored_command(&mut self, name: &str) -> bool {
        let normalized = Self::normalize_ignored(name);
        let before = self.stats.ignored_commands.len();
        self.stats.ignored_commands.retain(|c| c != &normalized);
        self.stats.ignored_commands.len() != before
    }

    /// Valide les valeurs connues de la configuration. Ne touche pas au disque ;
    /// renvoie la liste des problèmes détectés (vide = configuration saine).
    pub fn validate(&self) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();
        if self.search_limit == 0 {
            issues.push(ConfigIssue {
                level: IssueLevel::Error,
                message: "search_limit doit être strictement positif".to_string(),
            });
        }
        if crate::prune::parse_duration(&self.maintenance.auto_prune_after).is_err() {
            issues.push(ConfigIssue {
                level: IssueLevel::Error,
                message: format!(
                    "maintenance.auto_prune_after invalide : {:?} (ex : 180d, 6m, 1y)",
                    self.maintenance.auto_prune_after
                ),
            });
        }
        if self.sensitive_keywords.is_empty() {
            issues.push(ConfigIssue {
                level: IssueLevel::Warning,
                message: "sensitive_keywords est vide : aucune commande ne sera filtrée"
                    .to_string(),
            });
        }
        issues
    }
}

/// Clés de premier niveau reconnues dans le fichier TOML (pour signaler les
/// éventuelles coquilles lors de `mnemo config validate`).
const KNOWN_TOP_KEYS: &[&str] = &[
    "sensitive_keywords",
    "ignore_prefixes",
    "search_limit",
    "stats",
    "maintenance",
];

/// Charge et valide un fichier de configuration depuis le disque.
///
/// Renvoie la config désérialisée et la liste des problèmes (syntaxe TOML mise
/// à part, qui remonte en `Err`). Détecte aussi les clés de premier niveau
/// inconnues (probables coquilles) en avertissement.
pub fn load_and_validate(path: &Path) -> Result<(Config, Vec<ConfigIssue>)> {
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("lecture de la config {}", path.display()))?;
    let value: toml::Value = toml::from_str(&raw)
        .with_context(|| format!("syntaxe TOML invalide dans {}", path.display()))?;
    let cfg: Config = value
        .clone()
        .try_into()
        .with_context(|| format!("structure invalide dans {}", path.display()))?;

    let mut issues = cfg.validate();
    if let Some(table) = value.as_table() {
        for key in table.keys() {
            if !KNOWN_TOP_KEYS.contains(&key.as_str()) {
                issues.push(ConfigIssue {
                    level: IssueLevel::Warning,
                    message: format!("clé inconnue ignorée : {key:?}"),
                });
            }
        }
    }
    Ok((cfg, issues))
}

/// Sauvegarde le fichier de config existant avant écrasement.
///
/// Copie `config.toml` vers `config.toml.bak.AAAAMMJJ-HHMMSS`. N'a aucun effet
/// si le fichier n'existe pas encore. Garantit qu'on n'écrase jamais une
/// configuration sans en conserver une copie.
pub fn backup_existing(path: &Path) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    let stamp = crate::db::now_timestamp()
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>();
    let stamp = format!(
        "{}-{}",
        &stamp[..8.min(stamp.len())],
        &stamp[8.min(stamp.len())..]
    );
    let backup = path.with_file_name(format!(
        "{}.bak.{stamp}",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("config.toml")
    ));
    std::fs::copy(path, &backup)
        .with_context(|| format!("sauvegarde de {} vers {}", path.display(), backup.display()))?;
    harden_file(&backup);
    Ok(Some(backup))
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

/// Mode Unix privé attendu pour les fichiers sensibles (config, base, archives).
pub const SECRET_FILE_MODE: u32 = 0o600;
/// Mode Unix privé attendu pour les dossiers gérés par mnemo.
pub const SECRET_DIR_MODE: u32 = 0o700;

/// Resserre les permissions d'un fichier sensible à `600` (lecture/écriture
/// propriétaire uniquement).
///
/// N'altère jamais le contenu du fichier. Sur les plateformes non-Unix, ne fait
/// rien et ne renvoie pas d'erreur. L'opération est idempotente : la permission
/// n'est réécrite que si elle diffère déjà de `600`.
pub fn harden_file(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            let mut perms = meta.permissions();
            if perms.mode() & 0o777 != SECRET_FILE_MODE {
                perms.set_mode(SECRET_FILE_MODE);
                let _ = std::fs::set_permissions(path, perms);
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}

/// Resserre les permissions d'un dossier géré par mnemo à `700`.
///
/// Comme [`harden_file`], l'opération est best-effort, idempotente et sans effet
/// sur les plateformes non-Unix.
pub fn harden_dir(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(path) {
            if meta.is_dir() {
                let mut perms = meta.permissions();
                if perms.mode() & 0o777 != SECRET_DIR_MODE {
                    perms.set_mode(SECRET_DIR_MODE);
                    let _ = std::fs::set_permissions(path, perms);
                }
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}
