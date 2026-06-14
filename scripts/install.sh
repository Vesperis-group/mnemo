#!/usr/bin/env bash
#
# Installateur de mnemo (Linux / WSL).
#
# Par défaut, télécharge un binaire pré-compilé depuis les assets de la GitHub
# Release (rapide, sans toolchain Rust). Repli possible sur la compilation
# depuis les sources. Ne modifie jamais ~/.bashrc sans sauvegarde ni doublon.
#
# Usage rapide (installe la dernière release, binaire musl statique) :
#   curl -fsSL https://raw.githubusercontent.com/Vesperis-group/mnemo/main/scripts/install.sh | bash
#
# Choisir une version précise :
#   MNEMO_VERSION="v0.1.2" bash scripts/install.sh
#
# Choisir une cible précise (musl statique recommandé, ou GNU/glibc 2.35) :
#   MNEMO_TARGET="x86_64-unknown-linux-gnu-glibc2.35" bash scripts/install.sh
#
# Compiler depuis les sources au lieu de télécharger :
#   MNEMO_INSTALL_FROM_SOURCE=1 bash scripts/install.sh
#
# Options communes :
#   MNEMO_ASSUME_YES=1   confirme automatiquement (CI / non interactif)
#   MNEMO_NO_BASHRC=1    n'ajoute pas le bloc d'intégration .bashrc

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration (surchargée par variables d'environnement).
# ---------------------------------------------------------------------------
MNEMO_OWNER="${MNEMO_OWNER:-Vesperis-group}"
MNEMO_REPO="${MNEMO_REPO:-mnemo}"
MNEMO_REPO_URL="${MNEMO_REPO_URL:-https://github.com/${MNEMO_OWNER}/${MNEMO_REPO}.git}"
MNEMO_REPO_BRANCH="${MNEMO_REPO_BRANCH:-main}"
GITHUB_BASE="${MNEMO_GITHUB_BASE:-https://github.com}"
GITHUB_API="${MNEMO_GITHUB_API:-https://api.github.com}"

BIN_DIR="${HOME}/.local/bin"
BIN_PATH="${BIN_DIR}/mnemo"
BASHRC="${HOME}/.bashrc"

# Nettoyage des dossiers temporaires en fin d'exécution. (On évite un trap
# RETURN : il se déclenche aussi à la fin d'un `source`, ce qui supprimerait
# les fichiers trop tôt.)
_MNEMO_TMPDIRS=()
_mnemo_cleanup() {
    local d
    for d in "${_MNEMO_TMPDIRS[@]:-}"; do
        [ -n "${d}" ] && rm -rf "${d}"
    done
}
trap _mnemo_cleanup EXIT

# ---------------------------------------------------------------------------
# Affichage.
# ---------------------------------------------------------------------------
info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
ok()    { printf '\033[1;32m  ✓\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m  !\033[0m %s\n' "$*" >&2; }
die()   { warn "$*"; exit 1; }

# Demande oui/non. Respecte MNEMO_ASSUME_YES=1 pour le mode non interactif.
confirm() {
    local prompt="$1"
    if [ "${MNEMO_ASSUME_YES:-0}" = "1" ]; then
        return 0
    fi
    if [ ! -t 0 ]; then
        # Pas de TTY et pas d'auto-confirmation : on refuse par prudence.
        return 1
    fi
    local reply
    read -r -p "${prompt} [o/N] " reply
    case "${reply}" in
        o|O|y|Y|oui|Oui|yes|Yes) return 0 ;;
        *) return 1 ;;
    esac
}

# ---------------------------------------------------------------------------
# Téléchargement HTTP (curl ou wget).
# ---------------------------------------------------------------------------
http_to_file() {
    local url="$1" out="$2"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${url}" -o "${out}"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO "${out}" "${url}"
    else
        die "curl ou wget est requis pour l'installation distante."
    fi
}

http_to_stdout() {
    local url="$1"
    if command -v curl >/dev/null 2>&1; then
        curl -fsSL "${url}"
    elif command -v wget >/dev/null 2>&1; then
        wget -qO- "${url}"
    else
        die "curl ou wget est requis pour l'installation distante."
    fi
}

# ---------------------------------------------------------------------------
# Détection de la cible par défaut (musl statique, le plus compatible).
# ---------------------------------------------------------------------------
detect_target() {
    if [ -n "${MNEMO_TARGET:-}" ]; then
        printf '%s\n' "${MNEMO_TARGET}"
        return 0
    fi
    local machine
    machine="$(uname -m)"
    case "${machine}" in
        x86_64|amd64)  printf '%s\n' "x86_64-unknown-linux-musl" ;;
        aarch64|arm64) printf '%s\n' "aarch64-unknown-linux-musl" ;;
        *)
            warn "Architecture '${machine}' non reconnue ; tentative en x86_64 musl."
            printf '%s\n' "x86_64-unknown-linux-musl" ;;
    esac
}

# ---------------------------------------------------------------------------
# Résolution du tag de version (MNEMO_VERSION ou dernière release publiée).
# ---------------------------------------------------------------------------
resolve_tag() {
    if [ -n "${MNEMO_VERSION:-}" ]; then
        case "${MNEMO_VERSION}" in
            v*) printf '%s\n' "${MNEMO_VERSION}" ;;
            *)  printf 'v%s\n' "${MNEMO_VERSION}" ;;
        esac
        return 0
    fi
    local json tag
    json="$(http_to_stdout "${GITHUB_API}/repos/${MNEMO_OWNER}/${MNEMO_REPO}/releases/latest")" \
        || die "Impossible d'interroger la dernière release."
    tag="$(printf '%s' "${json}" | sed -n 's/.*"tag_name" *: *"\([^"]*\)".*/\1/p' | head -n1)"
    [ -n "${tag}" ] || die "Aucune release trouvée pour ${MNEMO_OWNER}/${MNEMO_REPO}."
    printf '%s\n' "${tag}"
}

# ---------------------------------------------------------------------------
# Étapes communes de finalisation (binaire déjà disponible à ${1}).
# Requiert que scripts/lib/bashrc.sh (${2}) ait été sourcé au préalable.
# ---------------------------------------------------------------------------
finalize_install() {
    local src_binary="$1"

    info "Installation du binaire dans ${BIN_PATH}"
    mkdir -p "${BIN_DIR}"
    install -m 0755 "${src_binary}" "${BIN_PATH}"
    ok "Installé : ${BIN_PATH}"

    case ":${PATH}:" in
        *":${BIN_DIR}:"*)
            ok "${BIN_DIR} est déjà dans le PATH" ;;
        *)
            warn "${BIN_DIR} n'est pas dans votre PATH."
            warn "Ajoutez ceci à votre ~/.bashrc :"
            warn '  export PATH="$HOME/.local/bin:$PATH"' ;;
    esac

    info "Initialisation de la configuration et de la base"
    "${BIN_PATH}" init >/dev/null
    ok "Configuration et base de données prêtes"

    if [ "${MNEMO_NO_BASHRC:-0}" = "1" ]; then
        warn "Intégration Bash ignorée (MNEMO_NO_BASHRC=1)."
    elif mnemo_bashrc_has_block "${BASHRC}"; then
        ok "Bloc mnemo déjà présent dans ${BASHRC} (aucune modification)"
    elif confirm "Ajouter l'intégration mnemo à ${BASHRC} ?"; then
        local snippet
        snippet="$("${BIN_PATH}" bashrc)"
        set +e
        mnemo_install_bashrc_block "${BASHRC}" "${snippet}"
        local rc=$?
        set -e
        if [ "${rc}" -eq 0 ]; then
            ok "Bloc mnemo ajouté à ${BASHRC} (sauvegarde créée)"
        else
            ok "Bloc mnemo déjà présent (aucune modification)"
        fi
    else
        warn "Intégration Bash non ajoutée. Vous pourrez la copier via : mnemo bashrc"
    fi

    echo
    info "Installation terminée. Prochaines étapes :"
    echo "    source ~/.bashrc"
    echo "    mnemo import"
    echo "    mnemo search"
}

# ---------------------------------------------------------------------------
# Mode TÉLÉCHARGEMENT : récupère un asset de la GitHub Release, vérifie le
# SHA-256, extrait et installe.
# ---------------------------------------------------------------------------
install_from_release() {
    local target tag asset base tmp
    target="$(detect_target)"
    tag="$(resolve_tag)"
    asset="mnemo-${tag}-${target}.tar.gz"
    base="${GITHUB_BASE}/${MNEMO_OWNER}/${MNEMO_REPO}/releases/download/${tag}"

    info "Téléchargement de ${asset} (${tag})"
    tmp="$(mktemp -d)"
    _MNEMO_TMPDIRS+=("${tmp}")

    http_to_file "${base}/${asset}" "${tmp}/${asset}" \
        || die "Échec du téléchargement de ${asset}. Vérifiez MNEMO_VERSION / MNEMO_TARGET."
    http_to_file "${base}/${asset}.sha256" "${tmp}/${asset}.sha256" \
        || die "Échec du téléchargement de la somme de contrôle."

    info "Vérification de l'intégrité (SHA-256)"
    ( cd "${tmp}" && sha256sum -c "${asset}.sha256" >/dev/null ) \
        || die "Somme de contrôle invalide : archive corrompue ou altérée."
    ok "Intégrité vérifiée"

    # Note (v0.7) : chaque release publie aussi une signature cosign
    # (<asset>.sigstore.json) et une attestation de provenance SLSA
    # (<asset>.provenance.sigstore.json). Leur vérification est, pour l'instant,
    # une étape MANUELLE documentée dans le README (« Vérifier l'intégrité d'une
    # release »). TODO v0.8 : vérification cosign optionnelle ici, sans jamais
    # affaiblir le contrôle SHA-256 ci-dessus.

    info "Extraction"
    tar -xzf "${tmp}/${asset}" -C "${tmp}"
    local extracted="${tmp}/mnemo-${tag}-${target}"
    [ -x "${extracted}/mnemo" ] || die "Binaire mnemo absent de l'archive."

    # Helpers .bashrc fournis par l'archive.
    # shellcheck source=scripts/lib/bashrc.sh
    source "${extracted}/scripts/lib/bashrc.sh"

    finalize_install "${extracted}/mnemo"
}

# ---------------------------------------------------------------------------
# Mode SOURCE : compile depuis le dépôt local (si présent) ou cloné.
# ---------------------------------------------------------------------------
install_from_source() {
    local project_dir script_dir=""
    if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
        script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    fi

    if [ -n "${script_dir}" ] && [ -f "${script_dir}/../Cargo.toml" ]; then
        project_dir="$(cd "${script_dir}/.." && pwd)"
        info "Compilation depuis les sources locales : ${project_dir}"
    else
        command -v git >/dev/null 2>&1 || die "git est requis pour cloner les sources."
        local tmp
        tmp="$(mktemp -d)"
        _MNEMO_TMPDIRS+=("${tmp}")
        info "Clonage de ${MNEMO_REPO_URL} (${MNEMO_REPO_BRANCH})"
        git clone --depth 1 --branch "${MNEMO_REPO_BRANCH}" "${MNEMO_REPO_URL}" "${tmp}/mnemo"
        project_dir="${tmp}/mnemo"
    fi

    command -v cargo >/dev/null 2>&1 || die "cargo introuvable. Installez Rust : https://rustup.rs"
    ( cd "${project_dir}" && cargo build --release )
    ok "Binaire compilé : ${project_dir}/target/release/mnemo"

    # shellcheck source=scripts/lib/bashrc.sh
    source "${project_dir}/scripts/lib/bashrc.sh"

    finalize_install "${project_dir}/target/release/mnemo"
}

# ---------------------------------------------------------------------------
# Point d'entrée.
# ---------------------------------------------------------------------------
main() {
    if [ "${MNEMO_INSTALL_FROM_SOURCE:-0}" = "1" ]; then
        install_from_source
    else
        install_from_release
    fi
}

main "$@"
