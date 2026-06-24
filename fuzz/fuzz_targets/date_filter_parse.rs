#![no_main]

//! Fuzz des parseurs de dates/durées de `mnemo`.
//!
//! Les filtres temporels (`--since`, `--until`, rétention de `prune`) acceptent
//! des durées relatives (`24h`, `7d`, `2w`, `3m`, `1y`) et des dates absolues
//! (`YYYY-MM-DD`). Sur une entrée arbitraire — invalide, très longue ou Unicode
//! — ces parseurs doivent renvoyer une erreur propre, jamais paniquer ni
//! déborder.

use libfuzzer_sys::fuzz_target;
use mnemo::{db, prune};

fuzz_target!(|spec: &str| {
    // `resolve_since` / `resolve_before` : pas de panique, entrée invalide → None.
    let _ = db::resolve_since(spec);
    let _ = db::resolve_before(spec);

    // `parse_duration` : pas de panique ni overflow, entrée invalide → Err.
    let _ = prune::parse_duration(spec);
});
