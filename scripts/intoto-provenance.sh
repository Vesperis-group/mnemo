#!/usr/bin/env bash
#
# Génère le fichier de provenance SLSA au format in-toto JSONL à partir des
# bundles Sigstore produits par scripts/sign-release.sh.
#
# Chaque ligne du fichier de sortie est l'enveloppe DSSE (champ `dsseEnvelope`)
# d'un bundle `.provenance.sigstore.json`. Ce format (une enveloppe DSSE JSON
# par ligne) est le format `.intoto.jsonl` standard reconnu par les outils SLSA
# et par OpenSSF Scorecard (check Signed-Releases).
#
# Ce script n'effectue aucune signature : il extrait uniquement les enveloppes
# déjà produites et vérifiées par scripts/sign-release.sh.
#
# Pré-requis :
#   - scripts/sign-release.sh doit avoir été exécuté (bundles .provenance.sigstore.json présents).
#   - jq doit être disponible.
#
# Variables d'environnement :
#   MNEMO_VERSION   version SemVer sans 'v' (ex. 1.6.21).
#                   Défaut : lue depuis Cargo.toml.
#
# Produit, à la racine du projet :
#   mnemo-v${MNEMO_VERSION}-provenance.intoto.jsonl
#     (une enveloppe DSSE par ligne, couvrant les 4 artefacts de release)
#
# Comportement fail-close (set -euo pipefail) : tout bundle manquant, tout
# échec d'extraction ou tout problème de sortie interrompt le script. En
# release, release-it avorte → aucun asset publié sans provenance valide.

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

# Mêmes artefacts couverts que scripts/sign-release.sh.
ASSETS=(
    "mnemo-v${MNEMO_VERSION}-x86_64-unknown-linux-gnu-glibc2.35.tar.gz"
    "mnemo-v${MNEMO_VERSION}-x86_64-unknown-linux-musl.tar.gz"
    "mnemo-v${MNEMO_VERSION}-sbom.cdx.json"
    "mnemo-v${MNEMO_VERSION}-checksums.txt"
)

INTOTO_FILE="mnemo-v${MNEMO_VERSION}-provenance.intoto.jsonl"
rm -f "${INTOTO_FILE}"

# Vérification préalable : tous les bundles doivent exister.
for asset in "${ASSETS[@]}"; do
    prov_bundle="${asset}.provenance.sigstore.json"
    if [ ! -f "${prov_bundle}" ]; then
        echo "Erreur : bundle de provenance introuvable : ${prov_bundle}" >&2
        echo "Exécutez scripts/sign-release.sh avant ce script." >&2
        exit 1
    fi
done

# Extraction des enveloppes DSSE (une par ligne = format .intoto.jsonl).
for asset in "${ASSETS[@]}"; do
    prov_bundle="${asset}.provenance.sigstore.json"
    jq -c '.dsseEnvelope' "${prov_bundle}" >> "${INTOTO_FILE}"
    echo "  Extrait : ${prov_bundle}"
done

# Gardes-fous : fichier non vide + suffixe correct.
if [ ! -s "${INTOTO_FILE}" ]; then
    echo "Erreur : ${INTOTO_FILE} est absent ou vide." >&2
    exit 1
fi

case "${INTOTO_FILE}" in
    *.intoto.jsonl) ;;
    *) echo "Erreur : le fichier de provenance doit finir par .intoto.jsonl" >&2; exit 1 ;;
esac

LINE_COUNT="$(wc -l < "${INTOTO_FILE}")"
echo "Provenance SLSA générée : ${INTOTO_FILE} (${LINE_COUNT} enveloppes DSSE)"
