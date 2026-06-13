#!/usr/bin/env bash
#
# Installateur de mnemo — compile, installe le binaire et propose l'intégration
# Bash. Conçu pour Linux / WSL. Ne modifie jamais un fichier sans sauvegarde.
#
# Deux modes, détectés automatiquement :
#   - LOCAL  : lancé depuis un dépôt cloné (build des sources présentes).
#   - DISTANT: lancé via `curl ... | bash` (clone MNEMO_REPO_URL puis build).
#
# Usage :
#   bash scripts/install.sh                         # local, interactif
#   MNEMO_ASSUME_YES=1 bash scripts/install.sh      # non interactif (CI)
#   MNEMO_NO_BASHRC=1 bash scripts/install.sh        # n'ajoute pas le bloc .bashrc
#
# Installation distante (quand le dépôt GitHub existera) :
#   curl -fsSL https://raw.githubusercontent.com/<USER>/mnemo/main/scripts/install.sh | bash
#   # ou en fixant explicitement l'URL du dépôt :
#   MNEMO_REPO_URL=https://github.com/<USER>/mnemo curl -fsSL .../install.sh | bash

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration du dépôt (PLACEHOLDER — à remplacer par l'URL réelle).
# ---------------------------------------------------------------------------
# Tant que le dépôt GitHub officiel n'existe pas, cette valeur reste un
# placeholder. Surchargez-la via la variable d'environnement MNEMO_REPO_URL.
MNEMO_REPO_URL="${MNEMO_REPO_URL:-https://github.com/REPLACE_ME/mnemo}"
MNEMO_REPO_BRANCH="${MNEMO_REPO_BRANCH:-main}"

# ---------------------------------------------------------------------------
# Petits utilitaires d'affichage.
# ---------------------------------------------------------------------------
info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
ok()    { printf '\033[1;32m  ✓\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m  !\033[0m %s\n' "$*" >&2; }

# ---------------------------------------------------------------------------
# Localisation du projet : mode LOCAL si les sources sont présentes, sinon
# mode DISTANT (clonage du dépôt dans un dossier temporaire).
# ---------------------------------------------------------------------------
detect_project_dir() {
    local script_dir=""
    if [ -n "${BASH_SOURCE[0]:-}" ] && [ -f "${BASH_SOURCE[0]}" ]; then
        script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    fi

    if [ -n "${script_dir}" ] && [ -f "${script_dir}/../Cargo.toml" ]; then
        # Mode LOCAL.
        ( cd "${script_dir}/.." && pwd )
        return 0
    fi

    # Mode DISTANT : clone du dépôt.
    if [ "${MNEMO_REPO_URL}" = "https://github.com/REPLACE_ME/mnemo" ]; then
        warn "Installation distante détectée mais MNEMO_REPO_URL n'est pas configuré."
        warn "Définissez l'URL du dépôt, par exemple :"
        warn "  MNEMO_REPO_URL=https://github.com/<USER>/mnemo bash install.sh"
        exit 1
    fi
    if ! command -v git >/dev/null 2>&1; then
        warn "git est requis pour l'installation distante."
        exit 1
    fi
    local tmp
    tmp="$(mktemp -d)"
    info "Clonage de ${MNEMO_REPO_URL} (${MNEMO_REPO_BRANCH})" >&2
    git clone --depth 1 --branch "${MNEMO_REPO_BRANCH}" "${MNEMO_REPO_URL}" "${tmp}/mnemo" >&2
    printf '%s\n' "${tmp}/mnemo"
}

PROJECT_DIR="$(detect_project_dir)"
SCRIPT_DIR="${PROJECT_DIR}/scripts"
# shellcheck source=scripts/lib/bashrc.sh
source "${SCRIPT_DIR}/lib/bashrc.sh"

BIN_DIR="${HOME}/.local/bin"
BIN_PATH="${BIN_DIR}/mnemo"
BASHRC="${HOME}/.bashrc"

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
# 1. Compilation en release.
# ---------------------------------------------------------------------------
info "Compilation de mnemo en mode release"
if ! command -v cargo >/dev/null 2>&1; then
    warn "cargo introuvable. Installez Rust : https://rustup.rs"
    exit 1
fi
( cd "${PROJECT_DIR}" && cargo build --release )
ok "Binaire compilé : ${PROJECT_DIR}/target/release/mnemo"

# ---------------------------------------------------------------------------
# 2. Installation du binaire dans ~/.local/bin.
# ---------------------------------------------------------------------------
info "Installation du binaire dans ${BIN_PATH}"
mkdir -p "${BIN_DIR}"
install -m 0755 "${PROJECT_DIR}/target/release/mnemo" "${BIN_PATH}"
ok "Installé : ${BIN_PATH}"

# ---------------------------------------------------------------------------
# 3. Vérification du PATH.
# ---------------------------------------------------------------------------
case ":${PATH}:" in
    *":${BIN_DIR}:"*)
        ok "${BIN_DIR} est déjà dans le PATH" ;;
    *)
        warn "${BIN_DIR} n'est pas dans votre PATH."
        warn "Ajoutez ceci à votre ~/.bashrc :"
        warn '  export PATH="$HOME/.local/bin:$PATH"' ;;
esac

# ---------------------------------------------------------------------------
# 4. Initialisation (config + base).
# ---------------------------------------------------------------------------
info "Initialisation de la configuration et de la base"
"${BIN_PATH}" init >/dev/null
ok "Configuration et base de données prêtes"

# ---------------------------------------------------------------------------
# 5. Intégration Bash (optionnelle, avec sauvegarde et anti-doublon).
# ---------------------------------------------------------------------------
if [ "${MNEMO_NO_BASHRC:-0}" = "1" ]; then
    warn "Intégration Bash ignorée (MNEMO_NO_BASHRC=1)."
elif mnemo_bashrc_has_block "${BASHRC}"; then
    ok "Bloc mnemo déjà présent dans ${BASHRC} (aucune modification)"
elif confirm "Ajouter l'intégration mnemo à ${BASHRC} ?"; then
    snippet="$("${BIN_PATH}" bashrc)"
    set +e
    mnemo_install_bashrc_block "${BASHRC}" "${snippet}"
    rc=$?
    set -e
    if [ "${rc}" -eq 0 ]; then
        ok "Bloc mnemo ajouté à ${BASHRC} (sauvegarde créée)"
    else
        ok "Bloc mnemo déjà présent (aucune modification)"
    fi
else
    warn "Intégration Bash non ajoutée. Vous pourrez la copier via : mnemo bashrc"
fi

# ---------------------------------------------------------------------------
# 6. Résumé final.
# ---------------------------------------------------------------------------
echo
info "Installation terminée. Prochaines étapes :"
echo "    source ~/.bashrc"
echo "    mnemo import"
echo "    mnemo search"
