#!/usr/bin/env bash
#
# Génère le SBOM (Software Bill of Materials) de mnemo au format CycloneDX JSON.
#
# Outil : cargo-cyclonedx (version EXACTE épinglée, installée par le Makefile ou
# le workflow CI ; ce script ne l'installe pas).
#
# Pré-requis : `cargo cyclonedx` disponible dans le PATH, Cargo.lock présent.
#
# Variables d'environnement (optionnelles, valeurs par défaut pour l'usage local) :
#   MNEMO_VERSION   version SemVer sans 'v' (ex. 0.7.0).
#                   Défaut : lue depuis Cargo.toml.
#
# Produit, à la racine du projet :
#   mnemo-v${MNEMO_VERSION}-sbom.cdx.json
#   mnemo-v${MNEMO_VERSION}-sbom.cdx.json.sha256
#
# Ces fichiers sont ignorés par git (.gitignore) : ils sont attachés à la
# GitHub Release, pas commités.
#
# Comportement fail-close : toute erreur (outil absent, génération KO, SBOM
# invalide, checksum non vérifiable) interrompt le script (set -e). En release,
# release-it avorte alors → aucune publication sans SBOM valide.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "${PROJECT_DIR}"

# --- Résolution de la version ----------------------------------------------
read_cargo_version() {
    sed -n 's/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n1
}

MNEMO_VERSION="${MNEMO_VERSION:-$(read_cargo_version)}"
MNEMO_VERSION="${MNEMO_VERSION#v}"

if [ -z "${MNEMO_VERSION}" ]; then
    echo "Erreur : MNEMO_VERSION introuvable (Cargo.toml ?)." >&2
    exit 1
fi

# --- Vérification de l'outillage -------------------------------------------
if ! cargo cyclonedx --version >/dev/null 2>&1; then
    echo "Erreur : cargo-cyclonedx est requis pour générer le SBOM." >&2
    echo "Installez la version épinglée : voir Makefile (make sbom) ou le workflow CI." >&2
    exit 1
fi

# --- Génération du SBOM ----------------------------------------------------
# --override-filename "<nom>.cdx" produit "<nom>.cdx.json" (l'outil ajoute .json).
# --all : liste l'ensemble des dépendances (pas seulement le premier niveau).
# --spec-version 1.5 : version de spécification CycloneDX produite.
BASENAME="mnemo-v${MNEMO_VERSION}-sbom.cdx"
SBOM="${BASENAME}.json"

rm -f "${SBOM}" "${SBOM}.sha256"

cargo cyclonedx \
    --format json \
    --spec-version 1.5 \
    --all \
    --override-filename "${BASENAME}" \
    --manifest-path Cargo.toml

if [ ! -f "${SBOM}" ]; then
    echo "Erreur : SBOM non généré : ${SBOM}" >&2
    exit 1
fi

# --- Validation du JSON et des champs CycloneDX ----------------------------
python3 - "${SBOM}" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, encoding="utf-8") as fh:
    doc = json.load(fh)

if doc.get("bomFormat") != "CycloneDX":
    raise SystemExit(f"SBOM invalide : bomFormat={doc.get('bomFormat')!r}")
if not doc.get("specVersion"):
    raise SystemExit("SBOM invalide : specVersion manquant")
if not isinstance(doc.get("components"), list) or not doc["components"]:
    raise SystemExit("SBOM invalide : aucune dépendance listée")
print(
    f"SBOM valide : bomFormat={doc['bomFormat']} "
    f"specVersion={doc['specVersion']} components={len(doc['components'])}"
)
PY

# --- Empreinte SHA-256 + auto-vérification ---------------------------------
sha256sum "${SBOM}" > "${SBOM}.sha256"
sha256sum -c "${SBOM}.sha256"

echo "SBOM créé : ${SBOM}"
cat "${SBOM}.sha256"
