<!--
Merci pour votre contribution ! Remplissez ce modèle pour faciliter la revue.
N'ajoutez aucun footer `Co-authored-by` ni mention d'assistant dans les commits.
-->

## Description

<!-- Que change cette PR et pourquoi ? Reliez l'issue concernée (ex. « Closes #123 »). -->

## Type de changement

- [ ] `fix` — correction de bug
- [ ] `feat` — nouvelle fonctionnalité
- [ ] `docs` — documentation
- [ ] `ci` / `chore` — outillage, CI, maintenance
- [ ] `refactor` / `test` — refactorisation ou tests

## Checklist

- [ ] La branche est à jour sur `main` et la CI passe au vert.
- [ ] **Tests exécutés** : `cargo fmt --check`, `cargo clippy -D warnings`,
      `cargo test --locked` (et fuzzing si pertinent).
- [ ] **Impact sécurité** considéré (entrées validées, pas d'affaiblissement des
      garde-fous gitleaks / CodeQL / audit / fuzzing).
- [ ] **Aucun secret** ni donnée personnelle introduit dans le code, les tests ou
      les corpus.
- [ ] **Aucune dépendance** Rust ou npm ajoutée sans justification ; *lockfiles*
      conservés ; actions GitHub épinglées par SHA complet.
- [ ] Commits sans footer `Co-authored-by` ni mention d'assistant.
- [ ] Documentation mise à jour si nécessaire.

## Notes pour les relecteurs

<!-- Points d'attention, choix techniques, limites connues. -->
