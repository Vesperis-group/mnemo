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

## Produit / UX (v0.9)

25. **La maintenance automatique est opt-in et protégée.**
    `mnemo maintenance run` ne supprime **rien** tant que
    `auto_prune_enabled = false` (valeur par défaut). Même activée, la purge
    exige `--yes` (ou une confirmation interactive) ; `--dry-run` ne modifie
    jamais la base, et une sauvegarde complète est créée avant suppression réelle
    si `auto_backup_before_prune = true`.
    (`src/maintenance.rs::tests::*`,
    `tests/v6_product.rs::{maintenance_status_et_dry_run_sans_suppression,
    maintenance_run_yes_supprime_les_anciennes}`)

26. **La configuration n'est jamais écrasée sans sauvegarde.**
    `mnemo config edit` sauvegarde l'ancienne config
    (`config.toml.bak.AAAAMMJJ-HHMMSS`) avant toute modification, puis revalide
    le résultat. `mnemo config validate` signale erreurs et avertissements sans
    rien modifier.
    (`src/config.rs::tests::*`,
    `tests/v6_product.rs::{config_show_path_validate,
    config_validate_detecte_une_erreur}`)

27. **Les filtres de recherche tolèrent les entrées invalides.**
    Une date `--since` / `--before` invalide (ou un `--since` de `stats`)
    n'interrompt pas la commande : le filtre est ignoré avec un avertissement,
    sans panique. Le format de sortie `--json` est **stable**.
    (`src/db.rs::tests::query_filter_combine_les_criteres`,
    `tests/v6_product.rs::{search_date_invalide_ne_panique_pas,
    search_json_est_stable, stats_since_invalide_ne_panique_pas}`)

28. **L'export compressé ne casse pas l'export existant.**
    `--gzip` ajoute une variante `.json.gz` / `.csv.gz` ; sans `--gzip`, l'export
    reste identique. Le contenu décompressé est conforme à l'export non
    compressé.
    (`src/export.rs::tests::{gzip_roundtrip_conserve_le_contenu,
    gz_path_ajoute_extension_si_absente}`,
    `tests/v6_product.rs::export_gzip_produit_un_fichier_valide`)

29. **La TUI quitte toujours sur `Ctrl+C`.**
    Quel que soit le mode (recherche, détails, filtres, confirmation),
    `Ctrl+C` quitte l'application.
    (`src/tui/events.rs::tests::ctrl_c_quitte_dans_tous_les_modes`)

## Permissions locales (v0.9.1)

30. **Les fichiers sensibles ne sont pas lisibles par les autres utilisateurs.**
    Sous Unix, la config (`config.toml`), la base (`history.db`) et les
    archives de sauvegarde (`*.tar.gz`) sont créées en `600` (lecture/écriture
    propriétaire uniquement) ; les dossiers gérés par mnemo (`~/.config/mnemo`,
    `~/.local/share/mnemo`, `…/backups`) sont resserrés à `700`. Le durcissement
    est centralisé (`config::harden_file` / `config::harden_dir`) et appliqué à
    la création (`Config::save`, `db::open`, `backup::create_backup`) ainsi qu'à
    `mnemo init` (idempotent). Sur les plateformes non-Unix, l'opération est un
    no-op et ne provoque jamais d'erreur.
    (`tests/doctor.rs::{init_cree_config_et_db_en_600,
    doctor_permissions_600_sont_correctes}`,
    `tests/v3_data_management.rs::backup_cree_une_archive_en_600`)

31. **`doctor` détecte et corrige les permissions trop ouvertes.**
    `mnemo doctor` signale en `WARN` toute config ou base plus ouverte que `600`
    (`Permissions trop ouvertes : … (actuel 644, attendu 600)`) sans la
    modifier. `mnemo doctor --fix` applique `chmod 600` (`[FIX]
    Permissions corrigées : … → 600`) sans jamais altérer le contenu des
    fichiers.
    (`tests/doctor.rs::{doctor_signale_config_trop_ouverte,
    doctor_signale_db_trop_ouverte, doctor_fix_resserre_config_a_600,
    doctor_fix_resserre_db_a_600}`)

## Permissions des sauvegardes (v0.9.2)

32. **Les sauvegardes existantes peuvent être remises en conformité.**
    `mnemo doctor` inspecte les archives `~/.local/share/mnemo/backups/*.tar.gz`
    et signale, de façon **agrégée et non bruyante**, celles qui sont trop
    ouvertes (`Backups trop ouverts : N fichier(s), attendu 600`) ; il n'affiche
    rien si aucune archive n'existe et un résumé `OK` discret si toutes sont en
    `600`. `mnemo doctor --fix` resserre à `600` toutes les archives trop
    ouvertes (y compris les sauvegardes créées avant v0.9.1) avec un unique
    résumé `[FIX] Permissions corrigées : N backup(s) → 600`, sans jamais lire,
    modifier ou supprimer le contenu des archives, et sans échouer si le dossier
    `backups` est absent.
    (`tests/doctor.rs::{doctor_signale_backups_trop_ouverts,
    doctor_aucun_warning_backups_si_dossier_absent,
    doctor_aucun_warning_backups_si_tous_en_600,
    doctor_fix_corrige_les_backups_existants,
    doctor_fix_compte_les_corrections_backups_dans_le_resume}`)

33. **La correction des permissions est rapportée explicitement.**
    `mnemo doctor --fix` rend visible chaque correction : la base est resserrée
    par `db::open` mais la correction est rapportée explicitement
    (`[FIX] Permissions corrigées : …/history.db → 600`) au lieu d'être
    silencieuse, et chaque correction (config, base, sauvegardes) est comptée
    dans le résumé `Corrections appliquées : X` et le total `FIX`.
    (`tests/doctor.rs::doctor_fix_reporte_explicitement_la_db`)

## Interface TUI (pré-v1.0)

34. **Le rendu de la TUI ne panique jamais, quelle que soit la taille.**
    Le rendu reste robuste sur terminal très petit (avertissement dédié),
    étroit (panneau de détails masqué), court (barre de synthèse masquée),
    aux dimensions minimales, sans sélection et avec des commandes très
    longues. Le formatage tronque sur les **caractères** (UTF-8 sûr) sans
    jamais découper un octet.
    (`src/tui/ui.rs::tests::{rendu_dimensions_standard_sans_panique,
    rendu_terminal_etroit_masque_details, rendu_terminal_court_masque_synthese,
    rendu_dimensions_minimales_sans_panique, rendu_liste_vide_sans_panique,
    rendu_commande_tres_longue_sans_panique, rendu_tous_les_overlays_sans_panique}`,
    `src/tui/format.rs::tests::*`)

35. **La synthèse (KPI) reflète fidèlement les commandes visibles.**
    Total, visibles, succès, échecs, taux d'échec, projets distincts et shell
    dominant sont calculés de façon déterministe ; succès et échecs portent sur
    les commandes **visibles** (après filtres), le total sur l'ensemble chargé.
    Le taux d'échec vaut `0.0` en l'absence de commande exécutée visible.
    (`src/tui/app.rs::tests::{overview_compte_succes_echecs_projets_et_shell,
    overview_taux_echec_nul_sans_commande_executee,
    overview_suit_les_filtres_visibles}`)

