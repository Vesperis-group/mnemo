//! Détection et redaction des secrets présents dans l'historique déjà stocké
//! (`mnemo secrets scan` / `mnemo secrets redact`).
//!
//! Le filtrage à l'enregistrement ([`crate::filter::is_sensitive`]) empêche de
//! stocker la plupart des commandes sensibles. Ce module traite l'historique
//! **déjà présent** : il repère les commandes suspectes et peut les redacter en
//! place, de façon non destructive (dry-run par défaut, sauvegarde obligatoire
//! avant toute écriture).
//!
//! Garantie anti-fuite : la sortie utilisateur et la version redactée ne
//! contiennent jamais la valeur sensible détectée. En cas de doute, la commande
//! entière est remplacée par `[REDACTED COMMAND]`.
//!
//! La détection est volontairement heuristique : elle vise une protection
//! raisonnable d'un historique shell, pas une exhaustivité parfaite.

use anyhow::{Context, Result};

use crate::db::SearchFilter;
use crate::{backup, config, confirm, db, filter};

/// Marqueur de valeur redactée.
const REDACTED: &str = "[REDACTED]";
/// Marqueur de commande entièrement redactée (non redactable proprement).
const REDACTED_COMMAND: &str = "[REDACTED COMMAND]";
/// Nombre d'exemples affichés en aperçu d'un dry-run.
const PREVIEW_SAMPLES: usize = 5;

/// Catégorie de détection d'un secret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    Password,
    Token,
    ApiKey,
    BearerToken,
    PrivateKey,
    CredentialUrl,
    EnvSecret,
    UnknownSensitive,
}

impl Category {
    /// Étiquette stable, utilisée à l'affichage et dans le JSON.
    pub fn label(self) -> &'static str {
        match self {
            Category::Password => "password",
            Category::Token => "token",
            Category::ApiKey => "api_key",
            Category::BearerToken => "bearer_token",
            Category::PrivateKey => "private_key",
            Category::CredentialUrl => "credential_url",
            Category::EnvSecret => "env_secret",
            Category::UnknownSensitive => "unknown_sensitive",
        }
    }
}

/// Résultat d'analyse d'une commande suspecte.
#[derive(Debug, Clone)]
pub struct Finding {
    /// Catégories détectées (triées, dédupliquées).
    pub categories: Vec<Category>,
    /// Version redactée de la commande, garantie sans valeur sensible.
    pub redacted: String,
}

impl Finding {
    /// Catégories jointes par `,` pour l'affichage en colonne.
    pub fn categories_label(&self) -> String {
        self.categories
            .iter()
            .map(|c| c.label())
            .collect::<Vec<_>>()
            .join(",")
    }
}

/// Indique si une commande a déjà été redactée (à ne pas retraiter).
fn already_redacted(command: &str) -> bool {
    command.contains(REDACTED) || command.contains(REDACTED_COMMAND)
}

/// Vrai si `c` peut composer un nom de variable d'environnement.
fn is_name_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Recherche insensible à la casse (ASCII) de `pat` dans `lc` à partir de `from`.
/// `lc` doit être la version `to_ascii_lowercase` (caractère à caractère) de la
/// commande, ce qui préserve l'alignement des indices.
fn find_ci(lc: &[char], pat: &[char], from: usize) -> Option<usize> {
    if pat.is_empty() || lc.len() < pat.len() {
        return None;
    }
    let last = lc.len() - pat.len();
    (from..=last).find(|&i| lc[i..i + pat.len()] == *pat)
}

/// Lit une valeur à partir de l'indice `start` : entre guillemets si la valeur
/// est quotée, sinon jusqu'à la prochaine espace. Renvoie la valeur et l'indice
/// suivant.
fn read_value(chars: &[char], start: usize) -> (String, usize) {
    let n = chars.len();
    if start >= n {
        return (String::new(), start);
    }
    let quote = chars[start];
    if quote == '"' || quote == '\'' {
        let mut j = start + 1;
        let mut value = String::new();
        while j < n && chars[j] != quote {
            value.push(chars[j]);
            j += 1;
        }
        let end = if j < n { j + 1 } else { j };
        (value, end)
    } else {
        let mut j = start;
        let mut value = String::new();
        while j < n && !chars[j].is_whitespace() {
            value.push(chars[j]);
            j += 1;
        }
        (value, j)
    }
}

/// Catégorie associée à un nom de variable, s'il est sensible.
fn assignment_category(name: &str) -> Option<Category> {
    let upper = name.to_ascii_uppercase();
    if upper.contains("PASSWORD") || upper.contains("PASSWD") || upper == "PASS" {
        Some(Category::Password)
    } else if upper.contains("API_KEY") || upper.contains("APIKEY") {
        Some(Category::ApiKey)
    } else if upper.contains("TOKEN") {
        Some(Category::Token)
    } else if upper.contains("SECRET")
        || upper.contains("ACCESS_KEY")
        || upper.contains("PRIVATE_KEY")
        || upper.contains("CREDENTIAL")
    {
        Some(Category::EnvSecret)
    } else {
        None
    }
}

/// Détecte les affectations `NOM=valeur` à nom sensible (`PASSWORD=`, `TOKEN=`,
/// `AWS_SECRET_ACCESS_KEY=`…). Gère les valeurs entre guillemets.
fn scan_assignments(chars: &[char]) -> Vec<(Category, String)> {
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i < n {
        if chars[i] == '=' {
            let mut start = i;
            while start > 0 && is_name_char(chars[start - 1]) {
                start -= 1;
            }
            if start < i {
                let name: String = chars[start..i].iter().collect();
                if let Some(cat) = assignment_category(&name) {
                    let (value, end) = read_value(chars, i + 1);
                    if !value.is_empty() {
                        out.push((cat, value));
                    }
                    i = end;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// Détecte les jetons `Authorization: Bearer <token>` (y compris dans `-H` /
/// `--header`).
fn scan_bearer(chars: &[char], lc: &[char]) -> Vec<String> {
    let pat: Vec<char> = "bearer".chars().collect();
    let n = chars.len();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = find_ci(lc, &pat, from) {
        let mut j = pos + pat.len();
        // Au moins une espace entre "Bearer" et le jeton.
        let mut saw_space = false;
        while j < n && chars[j].is_whitespace() {
            saw_space = true;
            j += 1;
        }
        if saw_space && j < n {
            let stop = chars[j];
            if stop != '"' && stop != '\'' {
                // Le jeton s'arrête à une espace ou à un guillemet fermant
                // (cas d'un en-tête entièrement quoté).
                let start = j;
                while j < n && !chars[j].is_whitespace() && chars[j] != '"' && chars[j] != '\'' {
                    j += 1;
                }
                let value: String = chars[start..j].iter().collect();
                if !value.is_empty() {
                    out.push(value);
                }
            }
        }
        from = pos + pat.len();
    }
    out
}

/// Détecte les URLs contenant des identifiants `scheme://user:password@host`.
/// Seul le mot de passe (après le premier `:` de la partie identifiants) est
/// renvoyé pour redaction.
fn scan_credential_urls(chars: &[char], lc: &[char]) -> Vec<String> {
    let pat: Vec<char> = "://".chars().collect();
    let n = chars.len();
    let mut out = Vec::new();
    let mut from = 0;
    while let Some(pos) = find_ci(lc, &pat, from) {
        let userinfo_start = pos + pat.len();
        let mut j = userinfo_start;
        let mut at = None;
        let mut colon = None;
        while j < n {
            let c = chars[j];
            if c == '@' {
                at = Some(j);
                break;
            }
            if c == '/' || c.is_whitespace() {
                break;
            }
            if c == ':' && colon.is_none() {
                colon = Some(j);
            }
            j += 1;
        }
        if let (Some(at), Some(colon)) = (at, colon) {
            if colon + 1 < at {
                let password: String = chars[colon + 1..at].iter().collect();
                if !password.is_empty() {
                    out.push(password);
                }
            }
        }
        from = userinfo_start;
    }
    out
}

/// Options longues sensibles reconnues et leur catégorie.
const CLI_OPTIONS: &[(&str, Category)] = &[
    ("--password", Category::Password),
    ("--pass", Category::Password),
    ("--token", Category::Token),
    ("--access-token", Category::Token),
    ("--api-key", Category::ApiKey),
    ("--api_key", Category::ApiKey),
    ("--apikey", Category::ApiKey),
    ("--secret", Category::EnvSecret),
];

/// Vrai si la position `i` est un début de mot (début de chaîne ou précédé d'une
/// espace).
fn at_word_start(chars: &[char], i: usize) -> bool {
    i == 0 || chars[i - 1].is_whitespace()
}

/// Détecte les options CLI sensibles `--password X`, `--token=X`, `--api-key X`.
fn scan_cli_options(chars: &[char], lc: &[char]) -> Vec<(Category, String)> {
    let n = chars.len();
    let mut out = Vec::new();
    for (opt, cat) in CLI_OPTIONS {
        let pat: Vec<char> = opt.chars().collect();
        let mut from = 0;
        while let Some(pos) = find_ci(lc, &pat, from) {
            from = pos + pat.len();
            if !at_word_start(chars, pos) {
                continue;
            }
            let after = pos + pat.len();
            if after < n && chars[after] == '=' {
                let (value, _) = read_value(chars, after + 1);
                if !value.is_empty() {
                    out.push((*cat, value));
                }
            } else if after < n && chars[after].is_whitespace() {
                let mut j = after;
                while j < n && chars[j].is_whitespace() {
                    j += 1;
                }
                // La valeur ne doit pas être une autre option.
                if j < n && chars[j] != '-' {
                    let (value, _) = read_value(chars, j);
                    if !value.is_empty() {
                        out.push((*cat, value));
                    }
                }
            }
        }
    }
    out
}

/// Clients de base de données acceptant un mot de passe attaché `-p<valeur>`.
fn mentions_db_client(lc: &[char]) -> bool {
    for client in ["mysql", "mariadb", "psql", "mysqldump"] {
        let pat: Vec<char> = client.chars().collect();
        if find_ci(lc, &pat, 0).is_some() {
            return true;
        }
    }
    false
}

/// Détecte le mot de passe attaché des clients SQL (`mysql -u root -p<valeur>`).
/// Volontairement restreint à ces clients pour éviter les faux positifs (`-p`
/// est courant ailleurs, par exemple `mkdir -p`).
fn scan_db_password(chars: &[char], lc: &[char]) -> Vec<String> {
    if !mentions_db_client(lc) {
        return Vec::new();
    }
    let n = chars.len();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 1 < n {
        if chars[i] == '-' && chars[i + 1] == 'p' && at_word_start(chars, i) {
            let after = i + 2;
            // Valeur attachée uniquement (`-pVALUE`), non vide et non une option.
            if after < n && !chars[after].is_whitespace() && chars[after] != '-' {
                let (value, end) = read_value(chars, after);
                if !value.is_empty() {
                    out.push(value);
                }
                i = end;
                continue;
            }
        }
        i += 1;
    }
    out
}

/// Détecte un fragment de clé privée embarqué.
fn contains_private_key(command: &str) -> bool {
    command.contains("PRIVATE KEY-----")
}

/// Déduplique une liste de catégories en préservant un ordre stable (tri).
fn dedup_sorted(mut cats: Vec<Category>) -> Vec<Category> {
    cats.sort();
    cats.dedup();
    cats
}

/// Analyse une commande et renvoie un [`Finding`] si elle paraît sensible.
///
/// `keywords` correspond à `config.sensitive_keywords` et alimente la détection
/// de repli `unknown_sensitive`, réutilisant la logique d'enregistrement.
pub fn analyze(command: &str, keywords: &[String]) -> Option<Finding> {
    if command.trim().is_empty() || already_redacted(command) {
        return None;
    }

    let chars: Vec<char> = command.chars().collect();
    let lc: Vec<char> = chars.iter().map(|c| c.to_ascii_lowercase()).collect();

    let mut categories: Vec<Category> = Vec::new();
    let mut values: Vec<String> = Vec::new();
    let mut force_full = false;

    if contains_private_key(command) {
        categories.push(Category::PrivateKey);
        force_full = true;
    }
    for value in scan_bearer(&chars, &lc) {
        categories.push(Category::BearerToken);
        values.push(value);
    }
    for value in scan_credential_urls(&chars, &lc) {
        categories.push(Category::CredentialUrl);
        values.push(value);
    }
    for (cat, value) in scan_assignments(&chars) {
        categories.push(cat);
        values.push(value);
    }
    for (cat, value) in scan_cli_options(&chars, &lc) {
        categories.push(cat);
        values.push(value);
    }
    for value in scan_db_password(&chars, &lc) {
        categories.push(Category::Password);
        values.push(value);
    }

    // Repli : aucune structure reconnue mais un mot-clé sensible est présent.
    if categories.is_empty() && filter::is_sensitive(command, keywords) {
        categories.push(Category::UnknownSensitive);
        force_full = true;
    }

    if categories.is_empty() {
        return None;
    }

    let redacted = build_redacted(command, &values, force_full);

    Some(Finding {
        categories: dedup_sorted(categories),
        redacted,
    })
}

/// Construit la version redactée d'une commande.
///
/// Remplace chaque valeur sensible par `[REDACTED]`. Si une redaction propre est
/// impossible (clé privée, repli `unknown_sensitive`, ou valeur résiduelle après
/// remplacement), la commande entière devient `[REDACTED COMMAND]`. Cette
/// fonction garantit que la sortie ne contient aucune valeur détectée.
fn build_redacted(command: &str, values: &[String], force_full: bool) -> String {
    if force_full || values.iter().all(|v| v.is_empty()) {
        return REDACTED_COMMAND.to_string();
    }

    let mut redacted = command.to_string();
    for value in values {
        if !value.is_empty() {
            redacted = redacted.replace(value.as_str(), REDACTED);
        }
    }

    // Garantie anti-fuite : aucune valeur détectée ne doit subsister.
    if values
        .iter()
        .any(|v| !v.is_empty() && redacted.contains(v.as_str()))
    {
        return REDACTED_COMMAND.to_string();
    }
    redacted
}

/// Une entrée suspecte prête à l'affichage ou à la redaction.
struct ScanResult {
    id: i64,
    created_at: String,
    finding: Finding,
}

/// Scanne la base et renvoie les commandes suspectes dont la redaction
/// modifierait réellement le texte (les commandes déjà redactées sont ignorées).
fn collect(conn: &rusqlite::Connection, keywords: &[String]) -> Result<(usize, Vec<ScanResult>)> {
    let records = db::all_commands(conn, &SearchFilter::default())?;
    let scanned = records.len();
    let mut results = Vec::new();
    for record in records {
        if let Some(finding) = analyze(&record.command, keywords) {
            if finding.redacted != record.command {
                results.push(ScanResult {
                    id: record.id,
                    created_at: record.created_at,
                    finding,
                });
            }
        }
    }
    // Ordre déterministe : par identifiant croissant.
    results.sort_by_key(|r| r.id);
    Ok((scanned, results))
}

/// `mnemo secrets scan` : repère les commandes sensibles sans rien modifier.
pub fn run_scan(limit: Option<usize>, json: bool) -> Result<()> {
    let cfg = config::Config::load()?;
    let conn = db::open(&config::db_path()?)?;
    let (_scanned, results) = collect(&conn, &cfg.sensitive_keywords)?;

    let shown: &[ScanResult] = match limit {
        Some(n) => &results[..n.min(results.len())],
        None => &results,
    };

    if json {
        let items: Vec<_> = shown
            .iter()
            .map(|r| {
                serde_json::json!({
                    "id": r.id,
                    "created_at": r.created_at,
                    "categories": r.finding.categories.iter().map(|c| c.label()).collect::<Vec<_>>(),
                    "redacted_command": r.finding.redacted,
                })
            })
            .collect();
        let value = serde_json::json!({
            "suspected": results.len(),
            "shown": shown.len(),
            "results": items,
        });
        println!("{}", serde_json::to_string_pretty(&value)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("Aucune commande potentiellement sensible détectée.");
        return Ok(());
    }

    println!(
        "Commandes potentiellement sensibles détectées : {}",
        results.len()
    );
    println!();
    println!("{:<6}  {:<19}  {:<20}  COMMANDE", "ID", "DATE", "TYPE");
    for r in shown {
        let date = r.created_at.get(0..19).unwrap_or(&r.created_at);
        println!(
            "{:<6}  {:<19}  {:<20}  {}",
            r.id,
            date,
            r.finding.categories_label(),
            r.finding.redacted
        );
    }
    if shown.len() < results.len() {
        println!();
        println!(
            "... {} résultat(s) supplémentaire(s) masqué(s) (utilisez --limit).",
            results.len() - shown.len()
        );
    }
    println!();
    println!("Aucune modification effectuée. Pour nettoyer : mnemo secrets redact --apply.");
    Ok(())
}

/// `mnemo secrets redact` : redacte les commandes sensibles déjà stockées.
///
/// Dry-run par défaut. `--apply` applique réellement, après une sauvegarde
/// obligatoire et (sauf `--yes`) une confirmation. En mode non interactif sans
/// `--yes`, l'application est refusée proprement.
pub fn run_redact(dry_run: bool, apply: bool, assume_yes: bool, _backup: bool) -> Result<()> {
    let _ = dry_run; // dry-run est le défaut ; le drapeau est explicite.
    let cfg = config::Config::load()?;
    let conn = db::open(&config::db_path()?)?;
    let (scanned, results) = collect(&conn, &cfg.sensitive_keywords)?;

    println!("Commandes scannées   : {scanned}");
    println!("Commandes suspectes  : {}", results.len());

    if results.is_empty() {
        println!("Rien à redacter.");
        return Ok(());
    }

    let preview = results.len().min(PREVIEW_SAMPLES);
    println!("Aperçu :");
    for r in &results[..preview] {
        println!(
            "  {:<6}  {:<20}  {}",
            r.id,
            r.finding.categories_label(),
            r.finding.redacted
        );
    }
    if results.len() > preview {
        println!("  ... et {} autre(s).", results.len() - preview);
    }

    if !apply {
        println!();
        println!("[dry-run] Aucune modification effectuée. Relancez avec --apply pour redacter.");
        return Ok(());
    }

    if !confirm::confirm(
        &format!("Redacter {} commande(s) suspecte(s) ?", results.len()),
        assume_yes,
    )? {
        println!("Redaction annulée.");
        return Ok(());
    }

    // Sauvegarde obligatoire avant toute écriture : en cas d'échec, on n'applique
    // rien.
    let safety = backup::create_backup(None)
        .context("sauvegarde préalable impossible : redaction annulée")?;
    println!("Sauvegarde automatique : {}", safety.path.display());

    let items: Vec<(i64, String)> = results
        .iter()
        .map(|r| (r.id, r.finding.redacted.clone()))
        .collect();
    let modified = db::apply_redactions(&conn, &items)?;

    println!();
    println!("Résumé :");
    println!("  Commandes scannées : {scanned}");
    println!("  Commandes suspectes: {}", results.len());
    println!("  Commandes modifiées: {modified}");
    println!("  Sauvegarde         : {}", safety.path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kw() -> Vec<String> {
        [
            "password",
            "token",
            "secret",
            "api_key",
            "bearer",
            "private_key",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    /// Vérifie qu'aucune valeur sensible ne subsiste dans la version redactée.
    fn assert_no_leak(redacted: &str, secrets: &[&str]) {
        for s in secrets {
            assert!(
                !redacted.contains(s),
                "fuite du secret {s:?} dans {redacted:?}"
            );
        }
    }

    #[test]
    fn detecte_bearer_token() {
        let f = analyze(
            "curl -H \"Authorization: Bearer abcdef123456\" https://api.example.com",
            &kw(),
        )
        .unwrap();
        assert!(f.categories.contains(&Category::BearerToken));
        assert_eq!(
            f.redacted,
            "curl -H \"Authorization: Bearer [REDACTED]\" https://api.example.com"
        );
        assert_no_leak(&f.redacted, &["abcdef123456"]);
    }

    #[test]
    fn detecte_affectation_password() {
        let f = analyze("export DB_PASSWORD=s3cr3t", &kw()).unwrap();
        assert!(f.categories.contains(&Category::Password));
        assert_eq!(f.redacted, "export DB_PASSWORD=[REDACTED]");
        assert_no_leak(&f.redacted, &["s3cr3t"]);
    }

    #[test]
    fn detecte_aws_secret_access_key() {
        let f = analyze("export AWS_SECRET_ACCESS_KEY=abcd1234", &kw()).unwrap();
        assert!(f.categories.contains(&Category::EnvSecret));
        assert_eq!(f.redacted, "export AWS_SECRET_ACCESS_KEY=[REDACTED]");
        assert_no_leak(&f.redacted, &["abcd1234"]);
    }

    #[test]
    fn detecte_url_avec_identifiants() {
        let f = analyze(
            "git clone https://user:p4ssw0rd@example.com/repo.git",
            &kw(),
        )
        .unwrap();
        assert!(f.categories.contains(&Category::CredentialUrl));
        assert_eq!(
            f.redacted,
            "git clone https://user:[REDACTED]@example.com/repo.git"
        );
        assert_no_leak(&f.redacted, &["p4ssw0rd"]);
    }

    #[test]
    fn detecte_option_cli_password() {
        let f = analyze("kubectl login --password hunter2 --user admin", &kw()).unwrap();
        assert!(f.categories.contains(&Category::Password));
        assert_eq!(
            f.redacted,
            "kubectl login --password [REDACTED] --user admin"
        );
        assert_no_leak(&f.redacted, &["hunter2"]);
    }

    #[test]
    fn detecte_option_cli_avec_egal() {
        let f = analyze("tool --token=abc123xyz", &kw()).unwrap();
        assert!(f.categories.contains(&Category::Token));
        assert_eq!(f.redacted, "tool --token=[REDACTED]");
        assert_no_leak(&f.redacted, &["abc123xyz"]);
    }

    #[test]
    fn detecte_mot_de_passe_mysql_attache() {
        let f = analyze("mysql -u root -psup3rs3cret", &kw()).unwrap();
        assert!(f.categories.contains(&Category::Password));
        assert_eq!(f.redacted, "mysql -u root -p[REDACTED]");
        assert_no_leak(&f.redacted, &["sup3rs3cret"]);
    }

    #[test]
    fn detecte_fragment_cle_privee() {
        let f = analyze("echo \"-----BEGIN OPENSSH PRIVATE KEY-----\" > id", &kw()).unwrap();
        assert!(f.categories.contains(&Category::PrivateKey));
        assert_eq!(f.redacted, "[REDACTED COMMAND]");
    }

    #[test]
    fn commande_non_sensible_inchangee() {
        assert!(analyze("ls -la", &kw()).is_none());
        assert!(analyze("git commit -m 'fix'", &kw()).is_none());
        assert!(analyze("mkdir -p build/out", &kw()).is_none());
    }

    #[test]
    fn commande_deja_redactee_inchangee() {
        assert!(analyze("export DB_PASSWORD=[REDACTED]", &kw()).is_none());
        assert!(analyze("[REDACTED COMMAND]", &kw()).is_none());
    }

    #[test]
    fn repli_mot_cle_redacte_toute_la_commande() {
        // "sshpass" n'a pas de structure reconnue mais est un mot-clé sensible.
        let f = analyze("sshpass -e ssh host", &["sshpass".to_string()]).unwrap();
        assert_eq!(f.categories, vec![Category::UnknownSensitive]);
        assert_eq!(f.redacted, "[REDACTED COMMAND]");
    }

    #[test]
    fn valeur_quotee_avec_espace() {
        let f = analyze("env PASSWORD=\"a b c\" run", &kw()).unwrap();
        assert_eq!(f.redacted, "env PASSWORD=\"[REDACTED]\" run");
        assert_no_leak(&f.redacted, &["a b c"]);
    }
}
