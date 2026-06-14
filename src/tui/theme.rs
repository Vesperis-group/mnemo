//! Palette centralisée et styles de la TUI « ops dashboard ».
//!
//! Toutes les couleurs et styles passent par ce module afin de garder un rendu
//! cohérent (au lieu de couleurs dispersées dans le code de rendu). Les couleurs
//! choisies restent lisibles sur un terminal sombre et dégradent proprement si
//! le terminal ne gère pas la couleur (les `Modifier` comme `BOLD`/`REVERSED`
//! restent visibles).

use ratatui::style::{Color, Modifier, Style};

/// Vert : succès, état sain.
pub const SUCCESS: Color = Color::Green;
/// Rouge : échec, action destructive.
pub const DANGER: Color = Color::Red;
/// Jaune : avertissement, dates / fenêtres temporelles.
pub const WARNING: Color = Color::Yellow;
/// Cyan : information, contexte projet.
pub const INFO: Color = Color::Cyan;
/// Gris atténué : libellés, métadonnées secondaires.
pub const MUTED: Color = Color::DarkGray;
/// Magenta : accent (branche Git, éléments mis en avant).
pub const ACCENT: Color = Color::Magenta;
/// Cyan vif : titres de blocs.
pub const TITLE: Color = Color::Cyan;
/// Gris : bordures.
pub const BORDER: Color = Color::DarkGray;

/// Style d'un titre de bloc (cyan gras).
pub fn title() -> Style {
    Style::default().fg(TITLE).add_modifier(Modifier::BOLD)
}

/// Style des bordures.
pub fn border() -> Style {
    Style::default().fg(BORDER)
}

/// Style d'un libellé atténué (clé de champ, métadonnée).
pub fn label() -> Style {
    Style::default().fg(MUTED)
}

/// Style d'une valeur lisible (texte principal).
pub fn value() -> Style {
    Style::default().fg(Color::Gray)
}

/// Style d'une valeur mise en avant (gras clair).
pub fn strong() -> Style {
    Style::default()
        .fg(Color::White)
        .add_modifier(Modifier::BOLD)
}

/// Style de la ligne sélectionnée dans la liste : inversion vidéo + accent, qui
/// reste lisible même sans support couleur.
pub fn selected() -> Style {
    Style::default()
        .add_modifier(Modifier::REVERSED | Modifier::BOLD)
        .fg(INFO)
}

/// Badge coloré : texte sur une couleur d'accent (gras).
pub fn badge(fg: Color) -> Style {
    Style::default().fg(fg).add_modifier(Modifier::BOLD)
}

/// Couleur associée à un code de sortie (vert si succès, rouge si échec, gris si
/// inconnu).
pub fn status_color(exit_code: Option<i64>) -> Color {
    match exit_code {
        Some(0) => SUCCESS,
        Some(_) => DANGER,
        None => MUTED,
    }
}
