#![no_main]

//! Fuzz du rendu Markdown de `mnemo` (échappement de texte arbitraire).
//!
//! Les helpers de `mnemo::mdfmt` encodent des commandes shell et des chemins
//! quelconques dans des documents Markdown (rapports `project`/`session`). Une
//! entrée malicieuse ne doit jamais provoquer de panique ni casser la structure
//! d'un tableau (aucun retour à la ligne ne doit subsister dans une cellule).

use libfuzzer_sys::fuzz_target;
use mnemo::mdfmt;

fuzz_target!(|data: &str| {
    // Aucune de ces fonctions ne doit paniquer sur une entrée arbitraire.
    let _ = mdfmt::longest_backtick_run(data);
    let _ = mdfmt::md_inline_code(data);

    let cell_text = mdfmt::md_table_cell_text(data);
    assert!(
        !cell_text.contains('\n') && !cell_text.contains('\r'),
        "une cellule de tableau ne doit pas contenir de retour à la ligne",
    );

    let cell_code = mdfmt::md_table_cell_code(data);
    assert!(
        !cell_code.contains('\n') && !cell_code.contains('\r'),
        "une cellule de code de tableau ne doit pas contenir de retour à la ligne",
    );

    let block = mdfmt::md_code_block(&[data.to_string()]);
    // Le bloc de code est toujours clôturé proprement.
    assert!(block.ends_with('\n'), "le bloc de code doit finir par un saut de ligne");
});
