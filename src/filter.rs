/// Détection des commandes sensibles.
///
/// Une commande est considérée sensible si elle contient l'un des mots-clés
/// (comparaison insensible à la casse).
pub fn is_sensitive(command: &str, keywords: &[String]) -> bool {
    let lower = command.to_lowercase();
    keywords.iter().any(|kw| {
        let kw = kw.trim();
        !kw.is_empty() && lower.contains(&kw.to_lowercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keywords() -> Vec<String> {
        [
            "password",
            "passwd",
            "token",
            "secret",
            "api_key",
            "bearer",
            "private_key",
            "sshpass",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect()
    }

    #[test]
    fn detecte_les_commandes_sensibles() {
        let kw = keywords();
        assert!(is_sensitive("export API_KEY=123", &kw));
        assert!(is_sensitive("mysql -u root -p password", &kw));
        assert!(is_sensitive("curl -H 'Authorization: Bearer abc'", &kw));
        assert!(is_sensitive("sshpass -p secret ssh host", &kw));
        assert!(is_sensitive("cat private_key.pem", &kw));
    }

    #[test]
    fn laisse_passer_les_commandes_normales() {
        let kw = keywords();
        assert!(!is_sensitive("ls -la", &kw));
        assert!(!is_sensitive("git commit -m 'fix'", &kw));
        assert!(!is_sensitive("cargo build --release", &kw));
    }

    #[test]
    fn ignore_les_mots_cles_vides() {
        let kw = vec!["".to_string(), "  ".to_string()];
        assert!(!is_sensitive("nimporte quelle commande", &kw));
    }
}
