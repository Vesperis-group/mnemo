#!/usr/bin/env bash
#
# Désinstallateur de mnemo : retire le binaire, propose de nettoyer le .bashrc
# et, sur confirmation explicite, supprime les données locales.
#
# Usage :
#   bash scripts/uninstall.sh
#   MNEMO_ASSUME_YES=1 bash scripts/uninstall.sh   # confirme bin + .bashrc (PAS les données)
#   MNEMO_PURGE=1 bash scripts/uninstall.sh         # supprime aussi les données

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib/bashrc.sh
source "${SCRIPT_DIR}/lib/bashrc.sh"

BIN_PATH="${HOME}/.local/bin/mnemo"
BASHRC="${HOME}/.bashrc"
CONFIG_DIR="${XDG_CONFIG_HOME:-${HOME}/.config}/mnemo"
DATA_DIR="${XDG_DATA_HOME:-${HOME}/.local/share}/mnemo"

info()  { printf '\033[1;34m==>\033[0m %s\n' "$*"; }
ok()    { printf '\033[1;32m  ✓\033[0m %s\n' "$*"; }
warn()  { printf '\033[1;33m  !\033[0m %s\n' "$*" >&2; }

confirm() {
    local prompt="$1"
    if [ "${MNEMO_ASSUME_YES:-0}" = "1" ]; then
        return 0
    fi
    if [ ! -t 0 ]; then
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
# 1. Binaire.
# ---------------------------------------------------------------------------
if [ -e "${BIN_PATH}" ]; then
    info "Suppression du binaire ${BIN_PATH}"
    rm -f "${BIN_PATH}"
    ok "Binaire supprimé"
else
    ok "Aucun binaire à supprimer (${BIN_PATH})"
fi

# ---------------------------------------------------------------------------
# 2. Bloc .bashrc (avec sauvegarde).
# ---------------------------------------------------------------------------
if mnemo_bashrc_has_block "${BASHRC}"; then
    if confirm "Retirer le bloc mnemo de ${BASHRC} ?"; then
        set +e
        mnemo_remove_bashrc_block "${BASHRC}"
        set -e
        ok "Bloc mnemo retiré (sauvegarde créée)"
    else
        warn "Bloc mnemo conservé dans ${BASHRC}"
    fi
else
    ok "Aucun bloc mnemo dans ${BASHRC}"
fi

# ---------------------------------------------------------------------------
# 3. Données locales : JAMAIS supprimées sans confirmation explicite.
#    MNEMO_ASSUME_YES ne suffit PAS : il faut MNEMO_PURGE=1 ou un "oui"
#    interactif dédié, afin d'éviter toute suppression accidentelle.
# ---------------------------------------------------------------------------
purge_data=0
if [ "${MNEMO_PURGE:-0}" = "1" ]; then
    purge_data=1
elif [ -d "${CONFIG_DIR}" ] || [ -d "${DATA_DIR}" ]; then
    warn "Données présentes :"
    [ -d "${CONFIG_DIR}" ] && warn "  config : ${CONFIG_DIR}"
    [ -d "${DATA_DIR}" ]   && warn "  données: ${DATA_DIR}"
    if [ -t 0 ]; then
        # Confirmation interactive dédiée (n'honore PAS MNEMO_ASSUME_YES).
        read -r -p "Supprimer DÉFINITIVEMENT ces données (historique inclus) ? [o/N] " reply
        case "${reply}" in
            o|O|y|Y|oui|Oui|yes|Yes) purge_data=1 ;;
            *) purge_data=0 ;;
        esac
    else
        warn "Mode non interactif : données conservées (utilisez MNEMO_PURGE=1 pour purger)."
    fi
fi

if [ "${purge_data}" = "1" ]; then
    rm -rf "${CONFIG_DIR}" "${DATA_DIR}"
    ok "Données supprimées"
else
    ok "Données conservées (config : ${CONFIG_DIR}, données : ${DATA_DIR})"
fi

echo
info "Désinstallation terminée."
echo "    Pensez à recharger votre shell : source ~/.bashrc"
