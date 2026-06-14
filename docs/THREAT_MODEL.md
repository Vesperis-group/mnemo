# Threat model de mnemo

Modèle de menaces de mnemo, outil **local-first** de navigation dans
l'historique shell. L'analyse suit une approche actifs → menaces → mitigations →
risques résiduels, adaptée à un outil mono-utilisateur sans composant serveur.

## 1. Périmètre et hypothèses

- mnemo s'exécute **localement**, sous le compte d'un utilisateur Linux.
- Pas de service réseau exposé, pas de multi-tenant, pas de privilèges root
  requis.
- Le seul trafic sortant provient de `update` / `upgrade` vers GitHub (HTTPS).
- L'attaquant pertinent est : une **archive malveillante** fournie à `restore`,
  un **asset de release altéré**, ou une **erreur de manipulation** de
  l'utilisateur entraînant une perte de données.

## 2. Actifs protégés

| Actif | Emplacement | Sensibilité |
| --- | --- | --- |
| `history.db` | `~/.local/share/mnemo/history.db` | Élevée (historique de commandes) |
| `config.toml` | `~/.config/mnemo/config.toml` | Moyenne (préférences, mots-clés sensibles) |
| Sauvegardes | `~/.local/share/mnemo/backups/` | Élevée (copies de la base) |
| `.bashrc` | `~/.bashrc` | Élevée (exécuté à chaque shell) |
| Binaire `mnemo` | `~/.local/bin/mnemo` | Élevée (code exécuté) |

## 3. Menaces, mitigations et risques résiduels

### M1 - Perte accidentelle de données
- **Menace** : suppression involontaire de la base / config via `delete`,
  `prune`, `maintenance run`, `restore` ou `uninstall`.
- **Mitigations** : `--dry-run` partout ; confirmation obligatoire (refus en
  non-interactif sans `--yes`) ; **sauvegarde automatique** avant toute action
  destructive ; `uninstall` conserve les données par défaut. Le nettoyage
  automatique (`maintenance run`) est **désactivé par défaut**
  (`auto_prune_enabled = false`), exige `--yes`, et crée une sauvegarde avant
  purge si `auto_backup_before_prune = true`.
- **Risque résiduel** : un utilisateur passant `--yes --purge` en connaissance
  de cause supprime ses données (une sauvegarde de sécurité reste créée avant).

### M2 - Archive de restauration malveillante
- **Menace** : `mnemo restore archive.tar.gz` où l'archive est forgée.
- **Mitigations** : extraction dans un **dossier temporaire** isolé ;
  validation de chaque entrée (cf. M3) ; ouverture de la base en **lecture
  seule** + validation (table `commands`, version de schéma) **avant**
  remplacement ; sauvegarde de sécurité préalable.
- **Risque résiduel** : une base SQLite valide mais au contenu trompeur peut
  être restaurée - mais elle ne s'exécute pas (données, pas du code).

### M3 - Path traversal dans une archive tar
- **Menace** : entrées `../evil`, `/etc/cron.d/x`, liens symboliques sortant de
  la racine d'extraction.
- **Mitigations** : `src/archive.rs::safe_unpack` valide **chaque** entrée
  (rejet des chemins absolus, des `..` et des cibles de liens hors racine)
  **avant** écriture ; `unpack_in` de la crate `tar` revalide (défense en
  profondeur) ; extraction limitée au tempdir.
- **Risque résiduel** : négligeable ; couvert par tests unitaires et
  d'intégration (`restore`, `upgrade`).

### M4 - Mise à niveau corrompue / downgrade
- **Menace** : asset de release tronqué, altéré, ou substitué.
- **Mitigations** : vérification **SHA-256** de l'archive **avant** extraction
  (toujours obligatoire et bloquante) ; **vérification de la signature Sigstore**
  de l'archive (`cosign verify-blob`) lorsque `cosign` est présent, en défense en
  profondeur ; HTTPS obligatoire ; le nouveau binaire est testé (`--version`)
  avant remplacement **atomique** ; sauvegarde des données avant bascule ; en cas
  d'échec, le binaire en place reste intact.
- **Modes de vérification de signature (v0.8)** :
  - *best-effort* (défaut) : si `cosign` est absent ou si le bundle est
    indisponible, l'opération continue après avertissement, le SHA-256 étant déjà
    garanti. Une signature **invalide** refuse toujours l'installation.
  - *strict* (`mnemo upgrade --require-signature`, `MNEMO_REQUIRE_SIGNATURE=1`
    pour `install.sh`) : l'installation est refusée si la signature ne peut pas
    être vérifiée (cosign absent, bundle manquant, signature invalide).
- **Risque résiduel** : confiance dans le `.sha256` publié par la release
  GitHub (même origine que l'asset). En mode best-effort, l'absence de `cosign`
  réduit la mise à niveau au seul contrôle SHA-256 (pas de vérification
  cryptographique d'origine) ; le mode strict élimine ce risque au prix d'une
  dépendance à `cosign`. La **provenance SLSA** n'est pas vérifiée
  automatiquement côté client (vérification manuelle documentée). Un compromis de
  l'organisation GitHub resterait hors de portée de ces mitigations.

### M5 - SHA invalide / format `.sha256` inattendu
- **Menace** : fichier `.sha256` mal formé ou condensat erroné.
- **Mitigations** : parsing strict (`parse_sha256_file`) ; toute non-concordance
  **refuse** l'installation. (`upgrade_sha_invalide_refuse_installation`)
- **Risque résiduel** : aucun connu.

### M6 - Suppression non confirmée
- **Menace** : suppression silencieuse depuis un script / pipe.
- **Mitigations** : en mode non interactif (stdin non TTY), toute opération
  destructive sans `--yes` est **refusée** avec un message clair.
- **Risque résiduel** : usage explicite de `--yes` en CI (intentionnel).

### M7 - Injection dans `.bashrc`
- **Menace** : corruption du `.bashrc` (exécuté à chaque shell) lors de
  l'installation / désinstallation du bloc d'intégration.
- **Mitigations** : bloc délimité par des marqueurs uniques ; **sauvegarde**
  du `.bashrc` avant modification ; retrait idempotent et borné aux marqueurs
  (`remove_bashrc_block`, testé idempotent).
- **Risque résiduel** : négligeable.

### M8 - Fuite de commandes sensibles
- **Menace** : mots de passe / tokens capturés dans l'historique.
- **Mitigations** : filtrage à l'import via mots-clés sensibles configurables ;
  la base reste locale ; aucune commande n'est transmise sur le réseau.
- **Risque résiduel** : une commande sensible non couverte par les mots-clés
  peut être stockée localement (jamais exfiltrée). L'utilisateur peut la
  supprimer (`delete`) ou enrichir `sensitive_keywords`.

### M9 - Erreur réseau GitHub
- **Menace** : indisponibilité, timeout, réponse inattendue lors de
  `update` / `upgrade`.
- **Mitigations** : erreurs contextualisées (statut HTTP vs transport), code de
  sortie non nul, aucun effet de bord destructif. `doctor` reste hors-ligne.
- **Risque résiduel** : aucun (mode dégradé propre).

### M10 - Exposition locale de l'historique via permissions trop ouvertes
- **Menace** : sur une machine multi-utilisateurs, un autre compte local lit la
  config, la base d'historique shell ou les archives de sauvegarde parce que ces
  fichiers ont été créés avec des permissions de groupe/autres (`644`, `664`…).
- **Mitigations** : sous Unix, config, `history.db` et archives `*.tar.gz` sont
  créées en `600` et les dossiers gérés en `700` (durcissement centralisé
  appliqué à la création et à `init`). `mnemo doctor` signale toute permission
  trop ouverte et `mnemo doctor --fix` la resserre à `600` sans toucher au
  contenu.
- **Risque résiduel** : un fichier déplacé/copié manuellement hors des chemins
  gérés, ou un `umask` exotique appliqué après coup, échappe au durcissement
  tant que `doctor --fix` n'est pas relancé ; les plateformes non-Unix reposent
  sur les ACL du système de fichiers.

## 4. Surface réseau

- **Sortant uniquement**, HTTPS, vers `api.github.com` et `github.com`
  (surchargeables via variables d'environnement pour les tests).
- Aucune écoute réseau. Aucune télémétrie.

## 5. Décisions de conception (local-first)

- **Pas de cloud, pas de compte, pas de télémétrie** : toutes les données
  restent sur la machine.
- **Sauvegarde avant destruction** : choix systématique privilégiant la
  récupérabilité sur la concision.
- **Signature des binaires : Sigstore (keyless), pas de GPG** : la chaîne de
  confiance repose sur HTTPS + SHA-256 (toujours obligatoire) publiés par la
  release GitHub, **complétée depuis la v0.8 par la vérification optionnelle de
  la signature Sigstore** de l'archive (`cosign verify-blob`, OIDC keyless, sans
  clé privée longue durée). La vérification est best-effort par défaut (pour ne
  pas casser les environnements minimaux sans `cosign`) et peut être rendue
  stricte (`--require-signature` / `MNEMO_REQUIRE_SIGNATURE=1`). GPG reste écarté
  (gestion de clés longue durée jugée plus lourde et moins auditable que le
  modèle keyless).
- **Linux uniquement** : réduit la surface et simplifie les hypothèses
  (permissions POSIX, `~/.bashrc`).

## 6. Vérification continue

- Tests unitaires et d'intégration couvrant chaque menace (cf. `INVARIANTS.md`).
- Chaîne DevSecOps : `cargo audit`, `cargo deny`, `cargo machete`, `gitleaks`
  (cf. `Makefile` cible `audit` et `.github/workflows/audit.yml`).
- Clippy en mode `-D warnings`, pas d'`unsafe` dans le code applicatif.

### Exceptions de supply chain documentées

- `RUSTSEC-2024-0436` (`paste`) est accepté temporairement : il s'agit d'une
  dépendance transitive via `ratatui`, signalée `unmaintained` (pas une
  vulnérabilité active). Suivi pour suppression lors d'une mise à jour future
  de Ratatui. Voir `deny.toml`, section `[advisories].ignore`.

## 7. Durcissement CI/CD et chaîne de release

Le pipeline GitHub Actions est conçu pour qu'**aucune release ne soit publiée
si un seul contrôle critique échoue** :

- **Gating strict.** Le workflow `release.yml` est séquencé en trois jobs :
  `quality` (fmt, clippy, tests, build glibc + musl) et `audit`
  (`cargo deny`, `cargo audit`, `gitleaks`) s'exécutent en amont ; le job
  `publish` déclare `needs: [quality, audit]` et `if: success()`. Si la qualité
  ou l'audit échoue, la publication n'est jamais atteinte. Aucun job critique
  n'utilise `continue-on-error`.
- **Permissions minimales.** Le workflow tourne en `contents: read` par défaut.
  Seul le job `publish` élève ses droits à `contents: write` et reçoit le token
  de publication ; les jobs de qualité et d'audit n'y ont jamais accès.
- **Checksums SHA-256 des assets.** `scripts/package-release.sh` génère pour
  chaque archive (`glibc` et `musl`) un fichier `.tar.gz.sha256` et le vérifie
  immédiatement (`sha256sum -c`). Un checksum manquant ou invalide interrompt
  le packaging et donc la publication. Les `.sha256` sont attachés à la Release
  aux côtés des archives (cf. `release-it.json`, liste d'assets).
- **Actions épinglées par SHA.** Toutes les actions tierces (`actions/checkout`,
  `actions/setup-node`, `Swatinem/rust-cache`) sont référencées par SHA de
  commit complet, avec le tag lisible en commentaire et la procédure de mise à
  jour (`git ls-remote`). Cela empêche le déplacement silencieux d'un tag
  flottant.
- **Versions de l'outillage figées (pas de canal flottant).**
  - **Rust** : `rust-toolchain.toml` épingle la version exacte du compilateur
    (`channel = "1.96.0"`), ses composants (`rustfmt`, `clippy`) et la cible
    `x86_64-unknown-linux-musl`. Le `rustup` pré-installé sur le runner lit ce
    fichier ; aucune action tierce n'installe la toolchain. Chaque job affiche
    `rustc --version` et `rustup show active-toolchain` comme preuve.
  - **Node.js** : `.node-version` épingle la version (`24.15.0`), consommée par
    `setup-node` via `node-version-file` (pas de `node-version: 20` ni `latest`).
  - **Outils d'audit Cargo** : `cargo-audit`, `cargo-deny`, `cargo-machete` sont
    installés en version exacte (`--version X.Y.Z --locked`) et leurs versions
    sont affichées en CI.
  - **Outils d'intégrité d'artefacts** : `cargo-cyclonedx` (SBOM) est installé
    en version exacte (`--version 0.5.9 --locked`) ; `cosign` (signature +
    provenance) est installé depuis un binaire dont l'empreinte SHA-256 est
    vérifiée (version `v3.1.1` épinglée). Leurs versions sont affichées en CI.
- **SBOM, signatures et provenance des artefacts.** Le job `publish` produit,
  via les hooks `after:bump` de release-it (donc **avant** la création de la
  release) :
  - un **SBOM CycloneDX JSON** (`scripts/generate-sbom.sh`, `cargo-cyclonedx`),
    validé (champs `bomFormat`/`specVersion`/`components`) et accompagné de son
    empreinte SHA-256 ;
  - un fichier de **checksums agrégés** couvrant les deux archives et le SBOM
    (`scripts/checksums-release.sh`), vérifié avant signature ;
  - pour chaque artefact, une **signature `cosign`** (keyless, OIDC ambiant
    GitHub Actions - aucun secret long terme) et une **attestation de
    provenance SLSA v1** (`cosign attest-blob`), toutes deux **re-vérifiées**
    par `cosign verify-blob` / `verify-blob-attestation`
    (`scripts/sign-release.sh`).

  Ces étapes sont **fail-close** (`set -euo pipefail`) : tout échec de
  génération, de signature, d'attestation ou de vérification interrompt
  release-it, donc **aucune release n'est créée**. Le keyless requiert
  `id-token: write`, accordé uniquement au job `publish`. Les bundles Sigstore
  (`*.sigstore.json`, `*.provenance.sigstore.json`) sont attachés à la Release.
- **Runners épinglés.** Les workflows utilisent `ubuntu-24.04` (et
  `ubuntu-22.04` là où le lien glibc 2.35 de l'asset GNU l'impose) plutôt que
  `ubuntu-latest`. Ces images GitHub restent maintenues et moins flottantes que
  `latest`, **mais ne sont pas immuables au digest** : c'est une limite
  résiduelle assumée (cf. § risques résiduels).
- **Téléchargements vérifiés.** Le binaire `gitleaks` est épinglé en version et
  son empreinte SHA-256 est contrôlée avant exécution (pas de `curl | bash`, pas
  d'installation sans checksum). Le binaire `cosign` (v3.1.1) est installé de
  la même façon : empreinte SHA-256 vérifiée avant installation.
- **Lockfiles obligatoires.** `Cargo.lock` et `package-lock.json` sont
  versionnés. La CI utilise `cargo fetch --locked` puis `cargo build/test/clippy
  --locked` (aucune mise à jour implicite de dépendances) et `npm ci`
  (jamais `npm install`), complété par `npm audit --omit=dev` avant publication.
- **Publication contrôlée.** La release réelle n'est déclenchée que par un push
  sur `main` (typiquement un merge de PR), après CI verte. En local,
  `make release-check` n'exécute que `release-it --dry-run` : aucune publication
  réelle ne peut partir d'un poste de développement.

### Limites résiduelles de la supply chain CI

- Les runners GitHub hébergés (`ubuntu-24.04`, `ubuntu-22.04`) sont des images
  **mises à jour en place** : leur contenu évolue sans changement de label et
  n'est pas épinglé à un digest. On accepte cette confiance dans l'infrastructure
  GitHub ; un durcissement supplémentaire (runners conteneurisés épinglés par
  digest) reste possible mais hors périmètre pour un projet mono-utilisateur.
- Le toolchain Rust est téléchargé par `rustup` via la version épinglée ; on fait
  confiance à l'infrastructure de distribution officielle de Rust (signée).
- La **provenance** atteste l'origine CI (dépôt, workflow, commit, run) mais
  n'est pas un niveau SLSA Build L3 formellement certifié : le build n'est pas
  isolé sur un constructeur dédié inviolable. Le prédicat reflète le contexte
  GitHub Actions ; il faut faire confiance à ce contexte.
- Depuis la **v0.8**, `install.sh` et `mnemo upgrade` vérifient
  **automatiquement la signature Sigstore** de l'archive (`cosign verify-blob`)
  lorsque `cosign` est présent, en plus du contrôle SHA-256 toujours obligatoire.
  La vérification est best-effort par défaut (avertissement si `cosign` est
  absent, pour préserver les environnements minimaux) et peut être rendue
  bloquante via `--require-signature` / `MNEMO_REQUIRE_SIGNATURE=1`. `cosign`
  n'est jamais téléchargé automatiquement par l'installateur. La vérification de
  la **provenance** (`*.provenance.sigstore.json`) reste, elle, une **étape
  manuelle** documentée côté utilisateur ; seule la signature de l'archive est
  contrôlée automatiquement.

