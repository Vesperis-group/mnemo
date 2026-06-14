#!/usr/bin/env bash
#
# Agrège les empreintes SHA-256 de TOUS les artefacts de release de mnemo dans
# un unique fichier de checksums, puis vérifie l'intégralité des empreintes.
#
# Pré-requis : les archives et le SBOM doivent déjà avoir été générés
# (scripts/package-release.sh et scripts/generate-sbom.sh).
#
# Variables d'environnement (optionnelles) :
#   MNEMO_VERSION   version SemVer sans 'v' (ex. 0.7.0).
#                   Défaut : lue depuis Cargo.toml.
#
# Produit, à la racine du projet :
#   mnemo-v${MNEMO_VERSION}-checksums.txt   (empreintes de tous les assets)
#
# Le fichier liste, par ligne « <sha256>  <nom> », les artefacts suivants :
#   - mnemo-v${VERSION}-x86_64-unknown-linux-gnu-glibc2.35.tar.gz
#   - mnemo-v${VERSION}-x86_64-unknown-linux-musl.tar.gz
#   - mnemo-v${VERSION}-sbom.cdx.json
#
# Comportement fail-close : si un artefact attendu est absent, ou si une
# empreinte ne se vérifie pas, le script échoue (set -e). En release, release-it
# avorte → aucune publication avec un jeu d'artefacts incomplet ou corrompu.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_DIR}"

read_cargo_version() {
    sed -n 's/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n1
}

MNEMO_VERSION="${MNEMO_VERSION:-$(read_cargo_version)}"
MNEMO_VERSION="${MNEMO_VERSION#v}"

if [ -z "${MNEMO_VERSION}" ]; then
    echo "Erreur : MNEMO_VERSION introuvable (Cargo.toml ?)." >&2
    exit 1
fi

CHECKSUMS="mnemo-v${MNEMO_VERSION}-checksums.txt"

# Liste EXACTE des artefacts attendus (mêmes noms que package-release.sh /
# generate-sbom.sh). Toute absence est fatale.
ASSETS=(
    "mnemo-v${MNEMO_VERSION}-x86_64-unknown-linux-gnu-glibc2.35.tar.gz"
    "mnemo-v${MNEMO_VERSION}-x86_64-unknown-linux-musl.tar.gz"
    "mnemo-v${MNEMO_VERSION}-sbom.cdx.json"
)

for asset in "${ASSETS[@]}"; do
    if [ ! -f "${asset}" ]; then
        echo "Erreur : artefact attendu introuvable : ${asset}" >&2
        echo "Générez d'abord les archives et le SBOM." >&2
        exit 1
    fi
done

rm -f "${CHECKSUMS}"

# Empreintes agrégées (un seul passage sha256sum sur tous les assets).
sha256sum "${ASSETS[@]}" > "${CHECKSUMS}"

# Vérification immédiate : recalcule et compare toutes les empreintes du fichier.
sha256sum -c "${CHECKSUMS}"

echo "Checksums agrégés : ${CHECKSUMS}"
cat "${CHECKSUMS}"
