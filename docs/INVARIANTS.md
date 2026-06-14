# Invariants métier de mnemo

Ce document liste les **règles invariantes** que mnemo doit respecter en
permanence. Toute évolution du code doit les préserver ; chaque invariant est
adossé à des tests automatisés (référencés entre parenthèses).

> mnemo est un outil **local-first** : aucune donnée ne quitte la machine, sauf
> l'interrogation explicite de l'API GitHub par `update` / `upgrade`.

## Données utilisateur

1. **`uninstall` conserve les données par défaut.**
   `mnemo uninstall` (sans `--purge`) retire uniquement le binaire et le bloc
   `.bashrc`. `config.toml`, `history.db` et les sauvegardes sont **conservés**.
   (`tests/v5_lifecycle.rs::uninstall_yes_supprime_binaire_conserve_donnees`)

2. **`uninstall --purge` demande confirmation et sauvegarde avant suppression.**
   Une sauvegarde de sécurité est créée hors du dossier de données, puis une
   confirmation explicite est exigée. En mode non interactif, `--yes` est requis.
   (`tests/v5_lifecycle.rs::uninstall_purge_yes_supprime_donnees_avec_sauvegarde`,
   `::uninstall_purge_non_interactif_sans_yes_refuse`)

3. **Toute désinstallation exige une confirmation.**
   Même sans `--purge`, retirer le binaire et l'intégration shell demande une
   confirmation interactive (`[y/N]`) ou `--yes`. En non interactif sans `--yes`,
   la commande refuse proprement (code de sortie non nul).
   (`src/lifecycle/uninstall.rs::tests::*`,
   `tests/v5_lifecycle.rs::uninstall_non_interactif_sans_yes_refuse`)

4. **`upgrade` conserve config + base + sauvegardes.**
   La mise à niveau ne remplace que le binaire ; elle ne touche jamais aux
   données. (`tests/v5_lifecycle.rs::upgrade_installe_nouveau_binaire`)

5. **`delete` et `prune` sont protégés.**
   Les deux disposent de `--dry-run`, exigent une confirmation (ou `--yes`) et
   créent une sauvegarde automatique avant suppression.
   (`tests/v3_data_management.rs::{delete_dry_run_ne_supprime_rien,
   prune_dry_run_ne_supprime_rien, ...}`)

6. **La TUI ne supprime jamais sans confirmation.**
   La suppression passe par un mode de confirmation explicite ; si la sauvegarde
   préalable échoue, rien n'est supprimé.
   (`src/tui/app.rs::tests::{suppression_confirmee_retire_l_element,
   suppression_annulee_ne_modifie_rien}`)

7. **`restore` valide l'archive avant remplacement.**
   L'archive est extraite dans un dossier temporaire, la base est ouverte en
   lecture seule et validée (table `commands`, version de schéma), et une
   sauvegarde de sécurité est créée avant tout remplacement.
   (`src/backup.rs`, `tests/v3_data_management.rs::restore_*`)

## Sécurité des mises à niveau

8. **`upgrade` vérifie le SHA-256 avant remplacement.**
   L'archive téléchargée est comparée à son `.sha256` **avant** extraction ; un
   condensat invalide refuse l'installation.
   (`tests/v5_lifecycle.rs::upgrade_sha_invalide_refuse_installation`)

9. **`upgrade` n'exécute jamais de script distant.**
   Aucun `curl | bash`. mnemo télécharge uniquement des assets de release
   (archive + `.sha256`) et remplace son propre binaire de façon atomique.

10. **Extraction d'archive sans path traversal.**
    L'extraction (`restore`, `upgrade`) valide chaque entrée : rejet des chemins
    absolus, des remontées `..` et des liens hors racine. Aucune écriture hors du
    dossier temporaire.
    (`src/archive.rs::tests::*`,
    `tests/v3_data_management.rs::restore_refuse_path_traversal_parent`,
    `tests/v5_lifecycle.rs::upgrade_refuse_path_traversal`)

## Réseau

11. **`doctor` reste hors-ligne par défaut.**
    Le diagnostic n'effectue aucun appel réseau.
    (`tests/doctor.rs`)

12. **`update` est la seule commande pouvant contacter GitHub sans effet
    destructif.** `upgrade` y accède aussi mais uniquement pour télécharger
    l'asset. Aucune autre commande ne sort sur le réseau.

13. **Les erreurs réseau sont gérées proprement.**
    Échec clair, code de sortie non nul, aucune corruption d'état.
    (`tests/v5_lifecycle.rs::{update_erreur_reseau_affichee_proprement,
    upgrade_erreur_reseau_propre}`)

## Confidentialité

14. **Les commandes sensibles ne sont pas importées.**
    L'import filtre les commandes correspondant aux mots-clés sensibles
    (mots de passe, tokens, clés…).
    (`src/filter.rs::tests::*`, `src/importer.rs::tests::*`)

15. **La base reste locale.**
    `history.db` vit sous `~/.local/share/mnemo/` (ou `XDG_DATA_HOME`). Elle
    n'est jamais transmise sur le réseau.

## Périmètre

16. **Linux uniquement, schéma SQLite stable.**
    Le schéma est versionné (`PRAGMA user_version`) et migré de façon
    idempotente et non destructive ; il n'est pas modifié sans nécessité.

## Intégrité des artefacts de release

> Invariants garantis par le pipeline (`release.yml`, `release-it.json`,
> `scripts/`), pas par la suite de tests Rust.

17. **Aucune release sans SBOM, signatures et provenance valides.**
    Le SBOM CycloneDX, les signatures `cosign` et les attestations de provenance
    SLSA v1 sont produits **et vérifiés** dans les hooks `after:bump`, donc
    **avant** la création de la GitHub Release. Tout échec (`set -euo pipefail`)
    avorte release-it : aucun artefact n'est publié.
    (`scripts/{generate-sbom,checksums-release,sign-release}.sh`)

18. **Signature keyless, sans secret long terme.**
    `cosign` signe via l'OIDC ambiant de GitHub Actions (`id-token: write` limité
    au job `publish`). Aucune clé privée n'est stockée dans le dépôt ni dans les
    secrets du projet.
    (`scripts/sign-release.sh`, `.github/workflows/release.yml`)

19. **Tout artefact publié est couvert par une empreinte SHA-256.**
    Chaque archive a son `.tar.gz.sha256`, le SBOM a son `.sha256`, et un fichier
    `*-checksums.txt` agrège les empreintes de tous les assets, vérifié avant
    signature.
    (`scripts/{package-release,generate-sbom,checksums-release}.sh`)
