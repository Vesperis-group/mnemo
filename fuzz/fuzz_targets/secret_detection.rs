#![no_main]

//! Fuzz de la détection/redaction de secrets de `mnemo` (`mnemo::secrets`).
//!
//! `analyze` inspecte une commande shell arbitraire et, si elle contient un
//! secret, renvoie une version redactée garantie sans valeur sensible. Les
//! invariants vérifiés ici :
//!   * aucune panique sur entrée arbitraire ;
//!   * une commande redactée contient toujours un marqueur de redaction ;
//!   * la redaction est idempotente (réanalyser une sortie redactée → `None`).
//!
//! Aucune valeur sensible réelle n'est utilisée : l'entrée est entièrement
//! générée par le fuzzer et les mots-clés ci-dessous sont des libellés publics.

use libfuzzer_sys::fuzz_target;
use mnemo::secrets;

fuzz_target!(|command: &str| {
    let keywords = [
        "password".to_string(),
        "secret".to_string(),
        "token".to_string(),
    ];

    if let Some(finding) = secrets::analyze(command, &keywords) {
        assert!(
            finding.redacted.contains("[REDACTED]") || finding.redacted == "[REDACTED COMMAND]",
            "une commande redactée doit porter un marqueur de redaction",
        );

        // Idempotence : une sortie déjà redactée ne doit plus rien révéler.
        assert!(
            secrets::analyze(&finding.redacted, &keywords).is_none(),
            "la redaction doit être idempotente",
        );
    }
});
