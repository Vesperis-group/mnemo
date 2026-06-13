#!/usr/bin/env bash
#
# Construit l'archive de release Linux x86_64 de mnemo.
#
# Pré-requis : le binaire release doit déjà être compilé
# (`cargo build --release`). Ce script ne recompile pas.
#
# Produit, à la racine du projet :
#   - mnemo-linux-x86_64.tar.gz
#   - mnemo-linux-x86_64.tar.gz.sha256
#
# Ces fichiers sont volontairement ignorés par git (.gitignore) : ils sont
# attachés à la GitHub Release, pas commités.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_DIR}"

BIN="target/release/mnemo"
if [ ! -x "${BIN}" ]; then
    echo "Erreur : ${BIN} introuvable. Lancez d'abord 'cargo build --release'." >&2
    exit 1
fi

STAGE="mnemo-linux-x86_64"
ARCHIVE="${STAGE}.tar.gz"

rm -rf "${STAGE}" "${ARCHIVE}" "${ARCHIVE}.sha256"

mkdir -p "${STAGE}/scripts/lib"
cp "${BIN}" "${STAGE}/"
cp README.md "${STAGE}/"
cp scripts/install.sh "${STAGE}/scripts/"
cp scripts/uninstall.sh "${STAGE}/scripts/"
cp scripts/lib/bashrc.sh "${STAGE}/scripts/lib/"

tar -czf "${ARCHIVE}" "${STAGE}"
sha256sum "${ARCHIVE}" > "${ARCHIVE}.sha256"

rm -rf "${STAGE}"

echo "Archive créée : ${ARCHIVE}"
cat "${ARCHIVE}.sha256"
