# Contribuer à mnemo

Merci de votre intérêt pour `mnemo` ! Les contributions sont les bienvenues :
corrections, documentation, tests, idées. Ce guide décrit le déroulé attendu.
Pour l'installation de l'environnement de développement et la liste complète des
cibles `make`, voir la section **Contribution** du [README](README.md#contribution).

## Avant de commencer

- En participant à ce projet, vous acceptez de respecter notre
  [Code de conduite](CODE_OF_CONDUCT.md).
- Le développement direct sur `main` est **interdit par convention** : toute
  modification passe par une **branche dédiée** puis une **Pull Request**.
- Pour un changement non trivial, ouvrez d'abord une **issue** afin d'en discuter
  le périmètre avant d'écrire du code.

## Ouvrir une issue

Utilisez les modèles fournis dans l'onglet *Issues* :

- **Bug report** : version (`mnemo version`), système (distribution, WSL ou
  natif), étapes de reproduction minimales, comportement attendu vs observé.
- **Feature request** : le besoin, le cas d'usage, et si possible une piste de
  solution.

N'incluez **jamais** de données personnelles réelles ni de secret dans une issue.
Pour une **vulnérabilité de sécurité**, ne pas ouvrir d'issue publique : suivez
[SECURITY.md](SECURITY.md).

## Proposer une Pull Request

1. Forkez le dépôt (ou créez une branche si vous avez les droits) ；
2. Créez une branche descriptive, par exemple `fix/doctor-json-exit-code` ou
   `feat/search-filters`.
3. Faites des commits petits et cohérents.
4. Lancez la porte de qualité locale (voir ci-dessous).
5. Ouvrez la PR en remplissant le modèle ; reliez l'issue concernée.
6. La PR doit être **à jour sur `main`** et voir sa CI passer au vert avant revue.

### Style de commit

Le projet suit les [Conventional Commits](https://www.conventionalcommits.org/) :
l'incrément de version de release en est dérivé. Exemples de préfixes :

- `feat:` nouvelle fonctionnalité ;
- `fix:` correction de bug ;
- `docs:` documentation ;
- `ci:` / `chore:` outillage, CI, maintenance ;
- `test:` ajout ou correction de tests ;
- `refactor:` refactorisation sans changement de comportement.

Gardez un titre court et impératif, et un corps explicatif si nécessaire.

### Règle : pas de `Co-authored-by` automatique

Les messages de commit ne doivent contenir que le titre et, si besoin, un corps
explicatif. **N'ajoutez aucun footer `Co-authored-by`** ni aucune mention
d'assistant ou de génération automatique. Si un outil en ajoute un, retirez-le
avant de committer.

## Porte de qualité locale

Avant d'ouvrir une PR, lancez (la CI reproduit ces vérifications) :

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked
cargo audit
cargo deny check
cargo machete
gitleaks detect --source . --redact --verbose
actionlint
bash -n scripts/*.sh scripts/lib/*.sh
shellcheck scripts/*.sh scripts/lib/*.sh
npm audit
make help
```

Raccourcis équivalents : `make check` (fmt + clippy + tests) et
`make security-full` (audit chaîne d'approvisionnement + ShellCheck + actionlint).
Un hook [`pre-commit`](https://pre-commit.com/) **optionnel** est fourni
(`.pre-commit-config.yaml`).

## Règles de sécurité

- Ne committez **jamais** de secret, jeton, mot de passe ou donnée personnelle.
- Validez et échappez les entrées non fiables ; les helpers Markdown
  (`src/mdfmt.rs`) et la redaction de secrets (`src/secrets.rs`) existent pour ça.
- Côté Rust, évitez `unwrap()`/`expect()` dans le code applicatif : propagez les
  erreurs proprement.
- N'affaiblissez aucun garde-fou de sécurité (gitleaks, CodeQL, `cargo-audit`,
  `cargo-deny`, `cargo-machete`, ShellCheck, actionlint, release-smoke, fuzzing)
  pour « faire passer » un changement.

## Dépendances et actions épinglées

- N'ajoutez **pas** de dépendance Rust ou npm sans justification claire (besoin,
  alternative, impact sécurité, impact maintenance).
- Conservez les *lockfiles* (`Cargo.lock`, `package-lock.json`) à jour ; ne les
  supprimez pas.
- Les **GitHub Actions sont épinglées par SHA de commit complet** (40 caractères)
  avec un commentaire `# vX.Y.Z`. Ne réintroduisez jamais de tag flottant
  (`@v4`, `@main`). Dependabot met à jour le SHA en conservant cette convention.

## Travailler sur le fuzzing

mnemo intègre une baseline `cargo-fuzz` (nightly requis **uniquement** pour le
fuzzing, jamais pour le build normal). Pour lancer une cible localement :

```bash
rustup toolchain install nightly --component rust-src
cargo +nightly install cargo-fuzz --version 0.13.2 --locked
cargo +nightly fuzz run mdfmt_escape -- -max_total_time=30
```

Les cibles, invariants et bonnes pratiques sont détaillés dans
[docs/FUZZING.md](docs/FUZZING.md). N'ajoutez pas de corpus contenant de vrais
secrets ; les valeurs de test doivent être manifestement factices.

## Signaler une vulnérabilité

Pour tout problème de sécurité, **ne créez pas d'issue publique**. Suivez la
procédure de signalement privé décrite dans [SECURITY.md](SECURITY.md)
(GitHub Security Advisories de préférence).

## Licence

En contribuant, vous acceptez que votre contribution soit distribuée sous la
licence [MIT](LICENSE) du projet.
