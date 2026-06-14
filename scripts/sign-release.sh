#!/usr/bin/env bash
#
# Signe et atteste (provenance) les artefacts de release de mnemo avec cosign,
# en mode KEYLESS (OIDC ambiant : aucun secret long terme), puis VÉRIFIE chaque
# signature et chaque attestation. Toute vérification KO interrompt le script.
#
# Outil : cosign (version EXACTE épinglée, installée par le workflow CI ; ce
# script ne l'installe pas).
#
# Contexte d'exécution : ce script est destiné à tourner dans le job `publish`
# de GitHub Actions, qui dispose de `id-token: write`. cosign détecte alors
# automatiquement le fournisseur OIDC `github-actions` et obtient un certificat
# Fulcio éphémère (keyless). En local (sans OIDC), la signature échouerait : ce
# script n'est donc PAS appelé par `make release-check` (dry-run release-it).
#
# Pré-requis : archives, SBOM et fichier de checksums déjà générés.
#
# Variables d'environnement :
#   MNEMO_VERSION                version SemVer sans 'v'. Défaut : Cargo.toml.
#   MNEMO_SIGN_IDENTITY_REGEXP   regex de l'identité du certificat attendue.
#                                Défaut : workflows du dépôt mnemo.
#   MNEMO_SIGN_OIDC_ISSUER       émetteur OIDC attendu.
#                                Défaut : token.actions.githubusercontent.com.
#
# Produit, pour CHAQUE artefact <asset> :
#   <asset>.sigstore.json              bundle de signature (cosign sign-blob)
#   <asset>.provenance.sigstore.json   bundle d'attestation SLSA provenance
#
# Comportement fail-close (set -euo pipefail) : si la signature, l'attestation
# ou l'une des vérifications échoue, le script s'arrête en erreur. En release,
# release-it avorte → aucune publication d'artefact non signé / non vérifié.

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

# Identité keyless attendue : par défaut, tout workflow du dépôt mnemo, signé
# via l'émetteur OIDC de GitHub Actions.
IDENTITY_REGEXP="${MNEMO_SIGN_IDENTITY_REGEXP:-^https://github.com/Vesperis-group/mnemo/\.github/workflows/.+@refs/heads/main$}"
OIDC_ISSUER="${MNEMO_SIGN_OIDC_ISSUER:-https://token.actions.githubusercontent.com}"

# Type de prédicat de provenance : SLSA Provenance v1 (predicateType
# https://slsa.dev/provenance/v1). cosign le reconnaît via --type slsaprovenance1.
PREDICATE_TYPE="slsaprovenance1"

# --- Vérification de l'outillage -------------------------------------------
if ! command -v cosign >/dev/null 2>&1; then
    echo "Erreur : cosign est requis pour signer et attester les artefacts." >&2
    echo "Installez la version épinglée : voir le workflow CI (job publish)." >&2
    exit 1
fi

# --- Artefacts à signer + attester -----------------------------------------
ASSETS=(
    "mnemo-v${MNEMO_VERSION}-x86_64-unknown-linux-gnu-glibc2.35.tar.gz"
    "mnemo-v${MNEMO_VERSION}-x86_64-unknown-linux-musl.tar.gz"
    "mnemo-v${MNEMO_VERSION}-sbom.cdx.json"
    "mnemo-v${MNEMO_VERSION}-checksums.txt"
)

for asset in "${ASSETS[@]}"; do
    if [ ! -f "${asset}" ]; then
        echo "Erreur : artefact attendu introuvable : ${asset}" >&2
        exit 1
    fi
done

# --- Prédicat de provenance SLSA v1 ----------------------------------------
# Construit à partir des variables GitHub Actions (présentes dans le job
# publish). En dehors de la CI, des valeurs neutres sont utilisées : ce script
# n'étant invoqué qu'en CI keyless, c'est une simple sécurité.
PREDICATE_FILE="$(mktemp)"
trap 'rm -f "${PREDICATE_FILE}"' EXIT

GH_SERVER="${GITHUB_SERVER_URL:-https://github.com}"
GH_REPO="${GITHUB_REPOSITORY:-Vesperis-group/mnemo}"
GH_WORKFLOW_REF="${GITHUB_WORKFLOW_REF:-${GH_REPO}/.github/workflows/release.yml@refs/heads/main}"
GH_SHA="${GITHUB_SHA:-unknown}"
GH_REF="${GITHUB_REF:-refs/heads/main}"
GH_RUN_ID="${GITHUB_RUN_ID:-0}"
GH_RUN_ATTEMPT="${GITHUB_RUN_ATTEMPT:-0}"

cat > "${PREDICATE_FILE}" <<JSON
{
  "buildDefinition": {
    "buildType": "https://github.com/Vesperis-group/mnemo/.github/workflows/release.yml",
    "externalParameters": {
      "workflow": {
        "ref": "${GH_REF}",
        "repository": "${GH_SERVER}/${GH_REPO}",
        "path": ".github/workflows/release.yml"
      }
    },
    "internalParameters": {
      "version": "${MNEMO_VERSION}"
    },
    "resolvedDependencies": [
      {
        "uri": "git+${GH_SERVER}/${GH_REPO}@${GH_REF}",
        "digest": { "gitCommit": "${GH_SHA}" }
      }
    ]
  },
  "runDetails": {
    "builder": {
      "id": "${GH_SERVER}/${GH_WORKFLOW_REF}"
    },
    "metadata": {
      "invocationId": "${GH_SERVER}/${GH_REPO}/actions/runs/${GH_RUN_ID}/attempts/${GH_RUN_ATTEMPT}"
    }
  }
}
JSON

# --- Signature + attestation -----------------------------------------------
for asset in "${ASSETS[@]}"; do
    sig_bundle="${asset}.sigstore.json"
    prov_bundle="${asset}.provenance.sigstore.json"

    rm -f "${sig_bundle}" "${prov_bundle}"

    echo ">>> Signature keyless : ${asset}"
    cosign sign-blob \
        --yes \
        --oidc-provider github-actions \
        --bundle "${sig_bundle}" \
        "${asset}"

    echo ">>> Attestation de provenance (SLSA v1) : ${asset}"
    cosign attest-blob \
        --yes \
        --oidc-provider github-actions \
        --predicate "${PREDICATE_FILE}" \
        --type "${PREDICATE_TYPE}" \
        --bundle "${prov_bundle}" \
        "${asset}"
done

# --- Vérification (fail-close) ---------------------------------------------
# Si une seule vérification échoue, set -e interrompt → release avortée.
for asset in "${ASSETS[@]}"; do
    sig_bundle="${asset}.sigstore.json"
    prov_bundle="${asset}.provenance.sigstore.json"

    echo ">>> Vérification de signature : ${asset}"
    cosign verify-blob \
        --bundle "${sig_bundle}" \
        --certificate-identity-regexp "${IDENTITY_REGEXP}" \
        --certificate-oidc-issuer "${OIDC_ISSUER}" \
        "${asset}"

    echo ">>> Vérification de provenance : ${asset}"
    cosign verify-blob-attestation \
        --bundle "${prov_bundle}" \
        --type "${PREDICATE_TYPE}" \
        --check-claims=true \
        --certificate-identity-regexp "${IDENTITY_REGEXP}" \
        --certificate-oidc-issuer "${OIDC_ISSUER}" \
        "${asset}"
done

echo "Signatures et attestations de provenance générées et vérifiées pour ${#ASSETS[@]} artefacts."
