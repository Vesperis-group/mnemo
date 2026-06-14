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
  `prune`, `restore` ou `uninstall`.
- **Mitigations** : `--dry-run` partout ; confirmation obligatoire (refus en
  non-interactif sans `--yes`) ; **sauvegarde automatique** avant toute action
  destructive ; `uninstall` conserve les données par défaut.
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
- **Mitigations** : vérification **SHA-256** de l'archive **avant** extraction ;
  HTTPS obligatoire ; le nouveau binaire est testé (`--version`) avant
  remplacement **atomique** ; sauvegarde des données avant bascule ; en cas
  d'échec, le binaire en place reste intact.
- **Risque résiduel** : confiance dans le `.sha256` publié par la release
  GitHub (même origine que l'asset). Pas de signature GPG des binaires
  (décision local-first, voir §5) - un compromis de l'organisation GitHub
  resterait hors de portée de cette mitigation.

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

## 4. Surface réseau

- **Sortant uniquement**, HTTPS, vers `api.github.com` et `github.com`
  (surchargeables via variables d'environnement pour les tests).
- Aucune écoute réseau. Aucune télémétrie.

## 5. Décisions de conception (local-first)

- **Pas de cloud, pas de compte, pas de télémétrie** : toutes les données
  restent sur la machine.
- **Sauvegarde avant destruction** : choix systématique privilégiant la
  récupérabilité sur la concision.
- **Pas de signature GPG des binaires** : la chaîne de confiance repose sur
  HTTPS + SHA-256 publiés par la release GitHub. Acceptable pour un outil
  local-first à faible surface ; une signature pourra être ajoutée
  ultérieurement sans changer le modèle.
- **Linux uniquement** : réduit la surface et simplifie les hypothèses
  (permissions POSIX, `~/.bashrc`).

## 6. Vérification continue

- Tests unitaires et d'intégration couvrant chaque menace (cf. `INVARIANTS.md`).
- Chaîne DevSecOps : `cargo audit`, `cargo deny`, `cargo machete`, `gitleaks`
  (cf. `Makefile` cible `audit` et `.github/workflows/audit.yml`).
- Clippy en mode `-D warnings`, pas d'`unsafe` dans le code applicatif.
