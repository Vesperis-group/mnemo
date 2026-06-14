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

## Vérification de signature côté client (v0.8)

> Le SHA-256 reste l'invariant **bloquant** de référence (invariant 8). La
> vérification Sigstore est une défense en profondeur additionnelle ; elle ne
> s'exécute **jamais** avant que le SHA-256 ait été validé.

20. **Le SHA-256 est vérifié avant toute étape de signature.**
    Si l'empreinte SHA-256 est invalide, l'installation échoue **avant** que la
    moindre vérification Sigstore ne soit tentée ; aucun message de signature
    n'est émis.
    (`tests/v5_lifecycle.rs::upgrade_signature_ignoree_si_sha_invalide`)

21. **Une signature présente mais invalide refuse toujours l'installation.**
    Quel que soit le mode (best-effort ou strict), si `cosign` est disponible et
    que la vérification du bundle Sigstore échoue, `upgrade` et `install.sh`
    refusent l'installation et laissent le binaire en place intact.
    (`tests/v5_lifecycle.rs::upgrade_signature_invalide_refuse`,
    `tests/scripts.rs::install_signature_invalide_refuse`)

22. **Mode best-effort : `cosign` absent ne bloque pas, mais avertit.**
    Par défaut, si `cosign` est absent ou si le bundle de signature est
    indisponible, l'installation continue (le SHA-256 ayant déjà été vérifié)
    après un avertissement explicite. `cosign` n'est jamais téléchargé
    automatiquement.
    (`tests/v5_lifecycle.rs::upgrade_sans_strict_continue_si_cosign_absent`,
    `tests/scripts.rs::install_signature_non_stricte_continue_si_cosign_absent`)

23. **Mode strict : aucune installation sans signature vérifiée.**
    Avec `mnemo upgrade --require-signature` ou `MNEMO_REQUIRE_SIGNATURE=1` pour
    `install.sh`, l'installation est **refusée** si `cosign` est absent, si le
    bundle est indisponible, ou si la signature est invalide. `update --upgrade`
    transmet le drapeau `--require-signature` à `upgrade`.
    (`tests/v5_lifecycle.rs::{upgrade_require_signature_refuse_si_cosign_absent,
    upgrade_strict_refuse_si_bundle_indisponible,
    update_upgrade_require_signature_transmet_le_flag}`,
    `tests/scripts.rs::{install_signature_stricte_refuse_si_cosign_absent,
    install_signature_stricte_refuse_si_bundle_indisponible}`)

24. **La provenance SLSA n'est pas vérifiée automatiquement.**
    En v0.8, seul le bundle de **signature** de l'archive (`*.sigstore.json`)
    est contrôlé côté client. La vérification de la provenance
    (`*.provenance.sigstore.json`) reste **manuelle** (documentée dans le
    README).
