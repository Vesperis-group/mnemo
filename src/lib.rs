//! Bibliothèque interne de `mnemo`.
//!
//! Le binaire `mnemo` (voir `src/main.rs`) est une fine couche d'orchestration
//! au-dessus de cette bibliothèque. Exposer les modules ici permet de tester et
//! de *fuzzer* la logique pure (rendu Markdown, détection de secrets, parsing
//! des bornes temporelles) sans dépendre d'une base SQLite réelle.
//!
//! L'API publique de ce crate est destinée à un usage interne (binaire et
//! cibles de fuzzing du dossier `fuzz/`) ; elle n'offre aucune garantie de
//! stabilité pour des consommateurs externes.

pub mod archive;
pub mod backup;
pub mod cli;
pub mod completions;
pub mod config;
pub mod confirm;
pub mod db;
pub mod doctor;
pub mod export;
pub mod filter;
pub mod gitctx;
pub mod importer;
pub mod init;
pub mod lifecycle;
pub mod list;
pub mod maintenance;
pub mod mdfmt;
pub mod migrations;
pub mod project;
pub mod prune;
pub mod secrets;
pub mod session;
pub mod shell;
pub mod show;
pub mod stats;
pub mod tui;
pub mod version;
