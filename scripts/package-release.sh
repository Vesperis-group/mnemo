#!/usr/bin/env bash
#
# Construit une archive de release Linux de mnemo, paramétrée par cible.
#
# Pré-requis : le binaire doit déjà être compilé (ce script ne compile pas).
#
# Variables d'environnement (toutes optionnelles, avec valeurs par défaut
# permettant l'usage local) :
#   MNEMO_VERSION       version SemVer sans 'v' (ex. 0.1.2).
#                       Défaut : lue depuis Cargo.toml.
#   MNEMO_TARGET_LABEL  étiquette de cible utilisée dans le nom de l'asset
#                       (ex. x86_64-unknown-linux-musl).
#                       Défaut : x86_64-unknown-linux-gnu.
#   MNEMO_BINARY_PATH   chemin du binaire mnemo à empaqueter.
#                       Défaut : target/release/mnemo.
#
# Produit, à la racine du projet :
#   mnemo-v${MNEMO_VERSION}-${MNEMO_TARGET_LABEL}.tar.gz
#   mnemo-v${MNEMO_VERSION}-${MNEMO_TARGET_LABEL}.tar.gz.sha256
#
# L'archive contient : mnemo, README.md, scripts/install.sh,
# scripts/uninstall.sh, scripts/lib/bashrc.sh.
#
# Ces fichiers sont volontairement ignorés par git (.gitignore) : ils sont
# attachés à la GitHub Release, pas commités.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_DIR}"

# --- Résolution des paramètres ---------------------------------------------
read_cargo_version() {
    # Première ligne `version = "x.y.z"` de la section [package].
    sed -n 's/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n1
}

MNEMO_VERSION="${MNEMO_VERSION:-$(read_cargo_version)}"
# Normalise : retire un éventuel préfixe 'v'.
MNEMO_VERSION="${MNEMO_VERSION#v}"
MNEMO_TARGET_LABEL="${MNEMO_TARGET_LABEL:-x86_64-unknown-linux-gnu}"
MNEMO_BINARY_PATH="${MNEMO_BINARY_PATH:-target/release/mnemo}"

if [ -z "${MNEMO_VERSION}" ]; then
    echo "Erreur : MNEMO_VERSION introuvable (Cargo.toml ?)." >&2
    exit 1
fi
if [ ! -x "${MNEMO_BINARY_PATH}" ]; then
    echo "Erreur : binaire introuvable : ${MNEMO_BINARY_PATH}" >&2
    echo "Compilez d'abord la cible voulue (cargo build --release [--target ...])." >&2
    exit 1
fi

# --- Construction de l'archive ---------------------------------------------
STAGE="mnemo-v${MNEMO_VERSION}-${MNEMO_TARGET_LABEL}"
ARCHIVE="${STAGE}.tar.gz"

rm -rf "${STAGE}" "${ARCHIVE}" "${ARCHIVE}.sha256"

mkdir -p "${STAGE}/scripts/lib"
install -m 0755 "${MNEMO_BINARY_PATH}" "${STAGE}/mnemo"
cp README.md "${STAGE}/"
cp scripts/install.sh "${STAGE}/scripts/"
cp scripts/uninstall.sh "${STAGE}/scripts/"
cp scripts/lib/bashrc.sh "${STAGE}/scripts/lib/"

tar -czf "${ARCHIVE}" "${STAGE}"
sha256sum "${ARCHIVE}" > "${ARCHIVE}.sha256"

rm -rf "${STAGE}"

echo "Archive créée : ${ARCHIVE}"
cat "${ARCHIVE}.sha256"
