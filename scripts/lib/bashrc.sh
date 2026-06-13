#!/usr/bin/env bash
# Bibliothèque partagée pour la gestion du bloc mnemo dans ~/.bashrc.
# Sourcée par scripts/install.sh et scripts/uninstall.sh, et testée par cargo.
#
# Aucune de ces fonctions ne modifie un fichier sans sauvegarde préalable.

# Marqueurs encadrant le bloc d'intégration ajouté au .bashrc.
MNEMO_BEGIN_MARKER="# >>> mnemo init >>>"
MNEMO_END_MARKER="# <<< mnemo init <<<"

# Vrai (0) si le bloc mnemo est déjà présent dans le fichier rc donné.
mnemo_bashrc_has_block() {
    local rc="$1"
    [ -f "$rc" ] && grep -qF "$MNEMO_BEGIN_MARKER" "$rc"
}

# Sauvegarde le fichier rc vers <rc>.mnemo.bak.YYYYMMDD-HHMMSS.
# Imprime le chemin de la sauvegarde sur stdout. No-op si le fichier n'existe pas.
mnemo_backup_bashrc() {
    local rc="$1"
    [ -f "$rc" ] || return 0
    local backup
    backup="${rc}.mnemo.bak.$(date +%Y%m%d-%H%M%S)"
    cp -p "$rc" "$backup"
    printf '%s\n' "$backup"
}

# Ajoute le bloc mnemo (snippet passé en $2) au fichier rc ($1), de façon
# idempotente et après sauvegarde.
#   Retour 0  : bloc ajouté.
#   Retour 10 : bloc déjà présent, rien fait.
mnemo_install_bashrc_block() {
    local rc="$1"
    local snippet="$2"

    if mnemo_bashrc_has_block "$rc"; then
        return 10
    fi

    mnemo_backup_bashrc "$rc" >/dev/null

    # Garantit que le fichier se termine par une ligne vide avant l'ajout.
    if [ -f "$rc" ] && [ -s "$rc" ] && [ -n "$(tail -c1 "$rc")" ]; then
        printf '\n' >>"$rc"
    fi

    {
        printf '%s\n' "$MNEMO_BEGIN_MARKER"
        printf '%s\n' "$snippet"
        printf '%s\n' "$MNEMO_END_MARKER"
    } >>"$rc"

    return 0
}

# Retire le bloc mnemo du fichier rc ($1), après sauvegarde.
#   Retour 0  : bloc retiré.
#   Retour 10 : aucun bloc présent, rien fait.
mnemo_remove_bashrc_block() {
    local rc="$1"

    if ! mnemo_bashrc_has_block "$rc"; then
        return 10
    fi

    mnemo_backup_bashrc "$rc" >/dev/null

    # Supprime toutes les lignes entre les marqueurs (inclus).
    local tmp
    tmp="$(mktemp)"
    awk -v b="$MNEMO_BEGIN_MARKER" -v e="$MNEMO_END_MARKER" '
        $0 == b { skip = 1; next }
        $0 == e { skip = 0; next }
        skip != 1 { print }
    ' "$rc" >"$tmp"
    mv "$tmp" "$rc"

    return 0
}
