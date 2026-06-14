//! Tests d'intégration des commandes de cycle de vie (`update`, `upgrade`,
//! `uninstall`).
//!
//! Tout s'exécute dans un HOME temporaire isolé (HOME + XDG_* + `MNEMO_BIN_PATH`)
//! et, pour le réseau, contre un petit serveur HTTP local (jamais Internet).
//! On ne touche donc jamais aux données ni au binaire réels de l'utilisateur.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::thread;

// --------------------------------------------------------------------------
// Outils communs.
// --------------------------------------------------------------------------

/// Construit une commande `mnemo` isolée dans un HOME temporaire.
fn mnemo(home: &Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_mnemo"));
    cmd.env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        // Empêche toute interaction et tout accès au vrai binaire installé.
        .env("MNEMO_BIN_PATH", home.join(".local/bin/mnemo"))
        // Par défaut, aucun cosign : la vérification Sigstore est « absente »
        // (best-effort), ce qui rend les tests déterministes quel que soit
        // l'hôte. Les tests de signature surchargent cette variable.
        .env("MNEMO_COSIGN_BIN", home.join(".no-cosign"));
    cmd
}

/// Écrit un faux `cosign` exécutable dans le HOME de test et renvoie son chemin.
/// `valid` décide du résultat de `verify-blob` (0 = valide, 1 = invalide) ;
/// `version` répond toujours 0 pour signaler que cosign est « disponible ».
fn fake_cosign(home: &Path, valid: bool) -> PathBuf {
    let path = home.join(".local/bin/fake-cosign");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let verify_exit = if valid { 0 } else { 1 };
    let script = format!(
        "#!/bin/sh\ncase \"$1\" in\n  version) exit 0 ;;\n  verify-blob) exit {verify_exit} ;;\n  *) exit 0 ;;\nesac\n"
    );
    std::fs::write(&path, script).unwrap();
    let mut perms = std::fs::metadata(&path).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    std::fs::set_permissions(&path, perms).unwrap();
    path
}

/// Crée un faux binaire installé et renvoie son chemin.
fn fake_bin(home: &Path) -> PathBuf {
    let bin = home.join(".local/bin/mnemo");
    std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
    std::fs::write(&bin, b"#!/bin/sh\necho 'mnemo factice'\n").unwrap();
    bin
}

/// Initialise config + base dans le HOME temporaire.
fn init(home: &Path) {
    assert!(mnemo(home).arg("init").output().unwrap().status.success());
}

fn config_dir(home: &Path) -> PathBuf {
    home.join(".config/mnemo")
}
fn data_dir(home: &Path) -> PathBuf {
    home.join(".local/share/mnemo")
}

/// Calcule le SHA-256 hexadécimal d'un contenu.
fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Construit une archive `.tar.gz` contenant `mnemo-<tag>-<target>/mnemo`.
fn make_archive(tag: &str, target: &str, script: &[u8]) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let mut header = tar::Header::new_gnu();
    header.set_size(script.len() as u64);
    header.set_mode(0o755);
    header.set_cksum();
    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(enc);
    let path = format!("mnemo-{tag}-{target}/mnemo");
    builder.append_data(&mut header, path, script).unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

/// Construit une archive `.tar.gz` malveillante dont l'entrée porte un chemin
/// `entry_name` arbitraire (header écrit directement, contournant la validation
/// du builder). Sert à vérifier le rejet du path traversal.
fn make_evil_archive(entry_name: &str) -> Vec<u8> {
    use flate2::write::GzEncoder;
    use flate2::Compression;
    let data = b"evil";
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o755);
    header.set_entry_type(tar::EntryType::Regular);
    {
        let gnu = header.as_gnu_mut().unwrap();
        let b = entry_name.as_bytes();
        gnu.name[..b.len()].copy_from_slice(b);
    }
    header.set_cksum();
    let enc = GzEncoder::new(Vec::new(), Compression::default());
    let mut builder = tar::Builder::new(enc);
    builder.append(&header, &data[..]).unwrap();
    builder.into_inner().unwrap().finish().unwrap()
}

// --------------------------------------------------------------------------
// Serveur HTTP de test (mono-thread, détaché, jamais Internet).
// --------------------------------------------------------------------------

struct Route {
    path: String,
    content_type: &'static str,
    body: Vec<u8>,
}

/// Démarre un serveur local servant les routes données. Renvoie l'URL de base
/// (`http://127.0.0.1:PORT`). Le thread est détaché : il vit le temps du test.
fn start_server(routes: Vec<Route>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            // Lit la ligne de requête : `GET /chemin HTTP/1.1`.
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            if reader.read_line(&mut line).is_err() {
                continue;
            }
            // Vide le reste des en-têtes.
            let mut header = String::new();
            while reader
                .read_line(&mut header)
                .map(|n| n > 0)
                .unwrap_or(false)
            {
                if header == "\r\n" || header == "\n" {
                    break;
                }
                header.clear();
            }
            let path = line.split_whitespace().nth(1).unwrap_or("/").to_string();
            let path = path.split('?').next().unwrap_or(&path).to_string();

            let response = routes.iter().find(|r| r.path == path);
            match response {
                Some(route) => {
                    let head = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
                        route.body.len(),
                        route.content_type
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(&route.body);
                }
                None => {
                    let body = b"not found";
                    let head = format!(
                        "HTTP/1.1 404 Not Found\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    let _ = stream.write_all(head.as_bytes());
                    let _ = stream.write_all(body);
                }
            }
            // Draine d'éventuelles données restantes pour une fermeture propre.
            let mut sink = Vec::new();
            let _ = reader.into_inner().read_to_end(&mut sink);
        }
    });
    base
}

/// Routes complètes pour une release mockée (latest + archive + sha256).
fn release_routes(
    tag: &str,
    target: &str,
    archive: &[u8],
    sha_override: Option<&str>,
) -> Vec<Route> {
    let sha = sha_override
        .map(|s| s.to_string())
        .unwrap_or_else(|| sha256_hex(archive));
    let archive_name = format!("mnemo-{tag}-{target}.tar.gz");
    let sha_body = format!("{sha}  {archive_name}\n");
    vec![
        Route {
            path: "/repos/test-owner/test-repo/releases/latest".into(),
            content_type: "application/json",
            body: format!("{{\"tag_name\":\"{tag}\",\"prerelease\":false}}").into_bytes(),
        },
        Route {
            path: format!("/test-owner/test-repo/releases/download/{tag}/{archive_name}"),
            content_type: "application/octet-stream",
            body: archive.to_vec(),
        },
        Route {
            path: format!("/test-owner/test-repo/releases/download/{tag}/{archive_name}.sha256"),
            content_type: "text/plain",
            body: sha_body.into_bytes(),
        },
    ]
}

/// Ajoute les variables d'environnement de redirection réseau à une commande.
fn with_mock<'a>(cmd: &'a mut Command, base: &str) -> &'a mut Command {
    cmd.env("MNEMO_GITHUB_API", base)
        .env("MNEMO_GITHUB_BASE", base)
        .env("MNEMO_OWNER", "test-owner")
        .env("MNEMO_REPO", "test-repo")
}

/// Route servant le bundle de signature `<archive>.sigstore.json` pour un tag.
fn signature_route(tag: &str, target: &str, body: &[u8]) -> Route {
    let archive_name = format!("mnemo-{tag}-{target}.tar.gz");
    Route {
        path: format!("/test-owner/test-repo/releases/download/{tag}/{archive_name}.sigstore.json"),
        content_type: "application/json",
        body: body.to_vec(),
    }
}

const TARGET: &str = "x86_64-unknown-linux-musl";

fn stdout(out: &Output) -> String {
    String::from_utf8_lossy(&out.stdout).to_string()
}
fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).to_string()
}

// --------------------------------------------------------------------------
// uninstall
// --------------------------------------------------------------------------

#[test]
fn uninstall_dry_run_ne_supprime_rien() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    let out = mnemo(home)
        .args(["uninstall", "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(
        bin.exists(),
        "le binaire ne doit pas être supprimé en dry-run"
    );
    assert!(config_dir(home).exists());
    assert!(data_dir(home).exists());
    assert!(stdout(&out).contains("simulation"));
}

#[test]
fn uninstall_purge_dry_run_ne_supprime_rien() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    let out = mnemo(home)
        .args(["uninstall", "--purge", "--dry-run"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(bin.exists());
    assert!(config_dir(home).exists());
    assert!(data_dir(home).exists());
}

#[test]
fn uninstall_yes_supprime_binaire_conserve_donnees() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    let out = mnemo(home).args(["uninstall", "--yes"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!bin.exists(), "le binaire doit être supprimé");
    assert!(config_dir(home).exists(), "config conservée");
    assert!(data_dir(home).exists(), "données conservées");
    assert!(stdout(&out).contains("Données conservées"));
}

#[test]
fn uninstall_purge_yes_supprime_donnees_avec_sauvegarde() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    let out = mnemo(home)
        .args(["uninstall", "--purge", "--yes"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert!(!bin.exists());
    assert!(!config_dir(home).exists(), "config supprimée");
    assert!(!data_dir(home).exists(), "données supprimées");
    // Une sauvegarde de sécurité a été créée hors du dossier de données.
    let has_backup = std::fs::read_dir(home)
        .unwrap()
        .flatten()
        .any(|e| e.file_name().to_string_lossy().starts_with("mnemo-backup-"));
    assert!(
        has_backup,
        "une sauvegarde de sécurité doit exister dans HOME"
    );
}

#[test]
fn uninstall_non_interactif_sans_yes_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    // Pas de --yes, entrée non interactive (pipe) : doit refuser proprement
    // avec un code de sortie non nul et ne RIEN supprimer.
    let out = mnemo(home).arg("uninstall").output().unwrap();
    assert!(
        !out.status.success(),
        "doit échouer (confirmation requise) : {}",
        stderr(&out)
    );
    assert!(
        stderr(&out).contains("Confirmation requise"),
        "message attendu, obtenu : {}",
        stderr(&out)
    );
    assert!(bin.exists(), "le binaire ne doit pas être supprimé");
    assert!(config_dir(home).exists());
    assert!(data_dir(home).exists());
}

#[test]
fn uninstall_purge_non_interactif_sans_yes_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    // Pas de --yes, entrée non interactive : la purge doit être refusée avec un
    // code non nul, et rien ne doit être supprimé.
    let out = mnemo(home).args(["uninstall", "--purge"]).output().unwrap();
    assert!(
        !out.status.success(),
        "doit échouer (confirmation requise) : {}",
        stderr(&out)
    );
    assert!(stderr(&out).contains("Confirmation requise"));
    assert!(
        config_dir(home).exists(),
        "config conservée (purge refusée)"
    );
    assert!(
        data_dir(home).exists(),
        "données conservées (purge refusée)"
    );
    // Le binaire ne doit pas avoir été supprimé puisque la purge est annulée
    // avant toute action.
    assert!(bin.exists());
}

// --------------------------------------------------------------------------
// update
// --------------------------------------------------------------------------

#[test]
fn update_signale_nouvelle_version() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let base = start_server(release_routes("v99.0.0", TARGET, b"x", None));

    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd.arg("update").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("v99.0.0"));
    assert!(s.contains("Mise à jour disponible"));
}

#[test]
fn update_json_structure() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    let base = start_server(release_routes("v99.0.0", TARGET, b"x", None));

    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd.args(["update", "--json"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("\"latest_version\": \"v99.0.0\""));
    assert!(s.contains("\"update_available\": true"));
    assert!(s.contains("\"asset_target\""));
}

#[test]
fn update_erreur_reseau_affichee_proprement() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    // Port très probablement fermé.
    let mut cmd = mnemo(home);
    cmd.env("MNEMO_GITHUB_API", "http://127.0.0.1:1")
        .env("MNEMO_OWNER", "test-owner")
        .env("MNEMO_REPO", "test-repo");
    let out = cmd.arg("update").output().unwrap();
    assert!(!out.status.success(), "doit échouer proprement");
    let e = stderr(&out);
    assert!(e.contains("Error"), "message d'erreur attendu : {e}");
}

#[test]
fn update_non_interactif_reste_check_only_sans_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();
    let base = start_server(release_routes("v99.0.0", TARGET, b"x", None));

    // stdin/stdout non interactifs (sortie capturée) : aucune proposition ne
    // doit apparaître, et rien ne doit être installé.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd.arg("update").output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("Mise à jour disponible"));
    assert!(
        s.contains("Lancez `mnemo upgrade`"),
        "l'indication classique doit rester : {s}"
    );
    assert!(
        !s.contains("Installer maintenant"),
        "aucun prompt ne doit être affiché en non interactif : {s}"
    );
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire inchangé");
}

#[test]
fn update_a_jour_aucune_proposition() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();
    // Version distante plus ancienne que celle installée → pas de mise à jour.
    let base = start_server(release_routes("v0.0.1", TARGET, b"x", None));

    // Même avec `--upgrade --yes`, aucune installation si rien de plus récent.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd.args(["update", "--upgrade", "--yes"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("mnemo est à jour"));
    assert!(
        !s.contains("Installer maintenant"),
        "aucun prompt si pas de mise à jour : {s}"
    );
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire inchangé");
}

#[test]
fn update_upgrade_yes_installe_si_disponible() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    let script = b"#!/bin/sh\necho 'mnemo 99.0.0'\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    // `update --upgrade --yes` : enchaîne l'upgrade sans prompt et remplace le
    // binaire, en réutilisant la logique de `mnemo upgrade`.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd.args(["update", "--upgrade", "--yes"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("Mise à jour disponible"));
    assert!(
        s.contains("Intégrité SHA-256 vérifiée"),
        "le chemin upgrade (avec vérif SHA-256) doit être emprunté : {s}"
    );
    assert_eq!(
        std::fs::read(&bin).unwrap(),
        script,
        "le binaire doit être remplacé via le chemin upgrade"
    );
    // Les données restent intactes.
    assert!(config_dir(home).exists());
    assert!(data_dir(home).exists());
}

#[test]
fn update_upgrade_sans_yes_non_interactif_n_installe_pas() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    // `--upgrade` sans `--yes`, en non interactif : la confirmation finale de
    // `upgrade` refuse proprement, donc rien n'est installé (consentement requis).
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd.args(["update", "--upgrade"]).output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(
        std::fs::read(&bin).unwrap(),
        before,
        "aucune installation sans consentement"
    );
}

// --------------------------------------------------------------------------
// upgrade
// --------------------------------------------------------------------------

#[test]
fn upgrade_dry_run_ne_remplace_rien() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    // --version explicite : aucun appel réseau nécessaire en dry-run.
    let out = mnemo(home)
        .args([
            "upgrade",
            "--dry-run",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire inchangé");
    assert!(stdout(&out).contains("Simulation"));
}

#[test]
fn upgrade_installe_nouveau_binaire() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    // Le « binaire » téléchargé est un script qui répond à --version.
    let script = b"#!/bin/sh\necho 'mnemo 99.0.0'\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let after = std::fs::read(&bin).unwrap();
    assert_eq!(after, script, "le binaire doit être remplacé");
    assert!(stdout(&out).contains("Intégrité SHA-256 vérifiée"));
    // Les données restent intactes.
    assert!(config_dir(home).exists());
    assert!(data_dir(home).exists());
}

#[test]
fn upgrade_sha_invalide_refuse_installation() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    // Somme volontairement fausse.
    let wrong = "0".repeat(64);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, Some(&wrong)));

    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(!out.status.success(), "l'installation doit être refusée");
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire inchangé");
    assert!(stderr(&out).contains("SHA-256"));
}

#[test]
fn upgrade_erreur_reseau_propre() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    // Serveur sans la route d'archive (404) → échec propre, binaire intact.
    let base = start_server(vec![]);
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert_eq!(
        std::fs::read(&bin).unwrap(),
        before,
        "binaire intact après échec"
    );
}

#[test]
fn upgrade_refuse_path_traversal() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    // Archive malveillante (chemin `../evil`), mais avec un SHA-256 VALIDE : on
    // vérifie que la vérification d'intégrité passe puis que l'extraction
    // refuse le path traversal - sans remplacer le binaire.
    let archive = make_evil_archive("../evil-upgrade.txt");
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "upgrade doit refuser une archive avec path traversal"
    );
    assert_eq!(
        std::fs::read(&bin).unwrap(),
        before,
        "binaire intact après rejet"
    );
    assert!(!std::env::temp_dir().join("evil-upgrade.txt").exists());
}

// --------------------------------------------------------------------------
// upgrade : vérification Sigstore (cosign) - mode best-effort et strict.
// --------------------------------------------------------------------------

#[test]
fn upgrade_require_signature_refuse_si_cosign_absent() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    // cosign absent (MNEMO_COSIGN_BIN par défaut pointe sur un binaire absent)
    // + mode strict : l'upgrade doit être refusé après la vérif SHA-256.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args(["upgrade", "--yes", "--require-signature"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "le mode strict sans cosign doit refuser l'upgrade"
    );
    let err = stderr(&out);
    assert!(
        err.contains("Signature Sigstore obligatoire") && err.contains("cosign"),
        "message strict attendu, obtenu : {err}"
    );
    assert_eq!(
        std::fs::read(&bin).unwrap(),
        before,
        "binaire intact (refus avant remplacement)"
    );
}

#[test]
fn upgrade_sans_strict_continue_si_cosign_absent() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    // cosign absent, mode normal : avertissement clair puis on continue grâce
    // au SHA-256 obligatoire ; le binaire est bien remplacé.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("Intégrité SHA-256 vérifiée"));
    assert!(
        s.contains("Signature Sigstore non vérifiée") && s.contains("cosign absent"),
        "avertissement best-effort attendu, obtenu : {s}"
    );
    assert_eq!(std::fs::read(&bin).unwrap(), script, "binaire remplacé");
}

#[test]
fn upgrade_signature_valide_installe() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let cosign = fake_cosign(home, true);

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let mut routes = release_routes("v99.0.0", TARGET, &archive, None);
    routes.push(signature_route("v99.0.0", TARGET, b"{\"fake\":\"bundle\"}"));
    let base = start_server(routes);

    // cosign présent (stub valide) + bundle servi : signature vérifiée, install.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    cmd.env("MNEMO_COSIGN_BIN", &cosign);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--require-signature",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("Intégrité SHA-256 vérifiée"));
    assert!(
        s.contains("Signature Sigstore vérifiée"),
        "confirmation de signature attendue, obtenu : {s}"
    );
    assert_eq!(std::fs::read(&bin).unwrap(), script, "binaire remplacé");
}

#[test]
fn upgrade_signature_invalide_refuse() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();
    let cosign = fake_cosign(home, false);

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let mut routes = release_routes("v99.0.0", TARGET, &archive, None);
    routes.push(signature_route("v99.0.0", TARGET, b"{\"fake\":\"bundle\"}"));
    let base = start_server(routes);

    // cosign présent mais vérification KO : refus, même hors mode strict.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    cmd.env("MNEMO_COSIGN_BIN", &cosign);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "une signature invalide doit refuser l'upgrade"
    );
    assert!(stderr(&out).contains("Signature Sigstore invalide"));
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire intact");
}

#[test]
fn upgrade_strict_refuse_si_bundle_indisponible() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();
    let cosign = fake_cosign(home, true);

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    // Pas de route de signature → bundle 404.
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    cmd.env("MNEMO_COSIGN_BIN", &cosign);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--require-signature",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "bundle indisponible en mode strict doit refuser l'upgrade"
    );
    assert!(stderr(&out).contains("indisponible"));
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire intact");
}

#[test]
fn upgrade_signature_ignoree_si_sha_invalide() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();
    let cosign = fake_cosign(home, true);

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    // SHA-256 volontairement faux : l'échec doit survenir AVANT toute étape de
    // signature, même cosign présent et mode strict.
    let bad_sha = "0".repeat(64);
    let mut routes = release_routes("v99.0.0", TARGET, &archive, Some(&bad_sha));
    routes.push(signature_route("v99.0.0", TARGET, b"{\"fake\":\"bundle\"}"));
    let base = start_server(routes);

    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    cmd.env("MNEMO_COSIGN_BIN", &cosign);
    let out = cmd
        .args([
            "upgrade",
            "--yes",
            "--require-signature",
            "--version",
            "v99.0.0",
            "--target",
            TARGET,
        ])
        .output()
        .unwrap();
    assert!(!out.status.success(), "SHA-256 invalide doit échouer");
    let combined = format!("{}{}", stdout(&out), stderr(&out));
    assert!(
        combined.contains("SHA-256"),
        "l'échec doit porter sur le SHA-256, obtenu : {combined}"
    );
    assert!(
        !combined.contains("Signature Sigstore"),
        "aucune étape de signature ne doit être atteinte si le SHA-256 échoue : {combined}"
    );
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire intact");
}

#[test]
fn update_upgrade_require_signature_transmet_le_flag() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    // `update --upgrade --require-signature` avec cosign absent : si le flag est
    // bien transmis au chemin upgrade, l'installation est refusée (mode strict).
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args(["update", "--upgrade", "--yes", "--require-signature"])
        .output()
        .unwrap();
    assert!(
        !out.status.success(),
        "le flag --require-signature doit être propagé à upgrade"
    );
    assert!(stderr(&out).contains("Signature Sigstore obligatoire"));
    assert_eq!(std::fs::read(&bin).unwrap(), before, "binaire intact");
}

#[test]
fn update_json_reste_check_only_avec_require_signature() {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path();
    init(home);
    let bin = fake_bin(home);
    let before = std::fs::read(&bin).unwrap();

    let script = b"#!/bin/sh\necho ok\n";
    let archive = make_archive("v99.0.0", TARGET, script);
    let base = start_server(release_routes("v99.0.0", TARGET, &archive, None));

    // `--json` : sortie machine, aucune installation ni vérif de signature,
    // même combiné à --upgrade/--require-signature.
    let mut cmd = mnemo(home);
    with_mock(&mut cmd, &base);
    let out = cmd
        .args(["update", "--json", "--upgrade", "--require-signature"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    let s = stdout(&out);
    assert!(s.contains("\"update_available\": true"));
    assert!(
        !s.contains("Signature Sigstore"),
        "le mode JSON ne doit déclencher aucune vérification de signature : {s}"
    );
    assert_eq!(
        std::fs::read(&bin).unwrap(),
        before,
        "binaire intact en --json"
    );
}
