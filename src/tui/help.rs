//! Texte d'aide affiché dans la TUI (overlay `?` / `F1`).

/// Lignes d'aide : `(touche, description)`. Une entrée `("", "")` matérialise
/// un séparateur visuel.
pub fn shortcuts() -> Vec<(&'static str, &'static str)> {
    vec![
        ("Navigation", ""),
        ("↑ / k", "élément précédent"),
        ("↓ / j", "élément suivant"),
        ("PageUp / PageDown", "page précédente / suivante"),
        ("Home / End", "premier / dernier élément"),
        (
            "Entrée",
            "sélectionner et imprimer la commande, puis quitter",
        ),
        ("Esc / q", "quitter"),
        ("", ""),
        ("Recherche & focus", ""),
        ("frappe", "recherche en direct (mode Search)"),
        ("Tab", "basculer le focus liste/détails"),
        ("r", "rafraîchir les résultats"),
        ("c", "copier la commande (presse-papiers ou tampon interne)"),
        ("?  / F1", "afficher/masquer cette aide"),
        ("Ctrl+C", "quitter"),
        ("", ""),
        ("Suppression", ""),
        ("d", "supprimer la commande sélectionnée"),
        ("y", "confirmer (crée d'abord une sauvegarde)"),
        ("n / Esc", "annuler"),
        ("", ""),
        ("Filtres", ""),
        ("f", "ouvrir/fermer le panneau de filtres"),
        ("Ctrl+P", "filtrer par le projet de la sélection"),
        ("Ctrl+B", "filtrer par la branche de la sélection"),
        ("Ctrl+D", "filtrer par le répertoire de la sélection"),
        ("Ctrl+L", "effacer tous les filtres"),
        (
            "(panneau) p / b / w",
            "projet / branche / dossier depuis la sélection",
        ),
        (
            "(panneau) s",
            "faire défiler le statut (tous/succès/échecs)",
        ),
        ("(panneau) c", "effacer les filtres"),
    ]
}
