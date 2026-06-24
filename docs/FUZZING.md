# Fuzzing

mnemo intègre une baseline de fuzzing avec
[`cargo-fuzz`](https://github.com/rust-fuzz/cargo-fuzz) (moteur libFuzzer). Le
fuzzing exerce des fonctions **pures** réellement sensibles avec des entrées
aléatoires afin de débusquer paniques, débordements et invariants cassés avant
qu'ils n'atteignent un utilisateur.

> Rust **nightly** et `cargo-fuzz` sont requis **uniquement** pour le fuzzing.
> Le build, les tests et les releases de mnemo restent sur la toolchain stable
> figée par [`rust-toolchain.toml`](../rust-toolchain.toml). Vous n'avez jamais
> besoin de nightly pour compiler ou utiliser mnemo.

## Pourquoi fuzzer ces fonctions

Les cibles ont été choisies parce qu'elles traitent des **entrées non fiables**
(commandes shell historisées, chemins, spécifications de filtres saisies par
l'utilisateur) et sont **pures** : pas de base de données, pas de réseau, pas de
shell, donc rapides et stables en CI.

| Cible | Module | Ce qui est fuzzé | Invariants vérifiés |
| --- | --- | --- | --- |
| `mdfmt_escape` | `src/mdfmt.rs` | Échappement Markdown de texte arbitraire (cellules de tableau, code en ligne, blocs de code). | Aucune panique ; une cellule de tableau ne contient jamais de retour à la ligne ; un bloc de code reste clôturé. |
| `secret_detection` | `src/secrets.rs` | `analyze` : détection puis redaction de secrets dans une commande. | Aucune panique ; une sortie redactée porte toujours un marqueur de redaction ; la redaction est idempotente. |
| `date_filter_parse` | `src/db.rs`, `src/prune.rs` | Parsing des durées (`24h`, `7d`, `2w`, `3m`, `1y`) et dates (`YYYY-MM-DD`) des filtres `--since`/`--until` et de la rétention `prune`. | Aucune panique ni débordement ; une entrée invalide renvoie une erreur propre. |

Aucune valeur sensible réelle n'est utilisée : les entrées sont générées par le
fuzzer et les éventuels mots-clés de test sont des libellés publics et factices.

## Installer cargo-fuzz

```bash
rustup toolchain install nightly --component rust-src
cargo +nightly install cargo-fuzz --version 0.13.2 --locked
```

`cargo-fuzz` s'appuie sur libFuzzer/AddressSanitizer ; un toolchain nightly est
nécessaire pour ces fonctionnalités instables.

## Lancer localement

Depuis la racine du dépôt :

```bash
# Compiler toutes les cibles.
cargo +nightly fuzz build

# Lancer une cible (Ctrl-C pour arrêter), ou borner la durée.
cargo +nightly fuzz run mdfmt_escape -- -max_total_time=30
cargo +nightly fuzz run secret_detection -- -max_total_time=30
cargo +nightly fuzz run date_filter_parse -- -max_total_time=30
```

Les entrées intéressantes découvertes (corpus) et les cas reproduisant un crash
(artifacts) restent **locaux** : ils sont ignorés par git
([`fuzz/.gitignore`](../fuzz/.gitignore)) et ne sont jamais versionnés.

Le crate `fuzz/` est un **workspace indépendant** : il n'est pas inclus dans
`cargo build` ni `cargo test` à la racine, et n'impose donc pas nightly au
développement normal.

## PR vs campagne planifiée

Le workflow [`fuzz.yml`](../.github/workflows/fuzz.yml) exécute le fuzzing en CI :

- **`pull_request` / `push` sur `main`** (filtrés par `paths`) : run court de
  **30 s par cible**, pour détecter rapidement une régression.
- **`schedule`** (dimanche 05:00 UTC) : campagne plus longue de **120 s par
  cible**.
- **`workflow_dispatch`** : exécution manuelle à la demande.

Les permissions sont réduites à `contents: read` ; aucun corpus externe n'est
téléchargé.

## Limites connues

- Les runs CI sont volontairement **courts** : ils constituent une baseline de
  non-régression, pas une campagne de fuzzing exhaustive.
- Sans corpus versionné, chaque run repart d'un corpus vide ; la couverture
  progresse pendant le run mais n'est pas conservée entre exécutions.
- Le fuzzing cible des fonctions pures ; les chemins nécessitant une base SQLite,
  le réseau ou un shell ne sont pas couverts ici (ils relèvent des tests
  d'intégration).

## Lien avec OpenSSF Scorecard

Le check **Fuzzing** d'[OpenSSF Scorecard](SCORECARD.md) détecte l'intégration
d'un outil de fuzzing reconnu. Cette baseline `cargo-fuzz` vise à faire passer ce
check de 0 à une valeur positive, tout en apportant une réelle valeur de
robustesse (et non un dossier cosmétique).
