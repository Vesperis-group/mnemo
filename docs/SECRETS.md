# Nettoyage des secrets de l'historique

`mnemo secrets` détecte et redacte les **commandes sensibles déjà présentes**
dans l'historique local. Il complète le filtrage appliqué à l'enregistrement :
ce dernier empêche de stocker la plupart des commandes sensibles, mais ne couvre
pas l'historique importé d'un `~/.bash_history` antérieur à l'installation ni les
commandes échappant aux mots-clés configurés.

## Principe

Deux sous-commandes, sans effet de bord réseau :

- `mnemo secrets scan` : analyse l'historique stocké et liste les commandes
  potentiellement sensibles. Elles sont **toujours affichées redactées**.
- `mnemo secrets redact` : redacte ces commandes en base. **Dry-run par défaut**,
  écriture uniquement avec `--apply`.

Aucune valeur sensible n'apparaît jamais en clair, ni dans la sortie texte, ni
dans la sortie JSON. En cas de doute sur la redaction propre d'une valeur, la
commande entière est remplacée par `[REDACTED COMMAND]`.

## Détection

La détection est volontairement **heuristique** : elle vise une protection
raisonnable d'un historique shell, pas une exhaustivité parfaite. Elle ne dépend
d'aucune bibliothèque externe. Sont reconnus :

| Catégorie | Exemple détecté | Valeur redactée |
| --- | --- | --- |
| `bearer_token` | `Authorization: Bearer <jeton>` | le jeton |
| `password` | `--password X`, `PASSWORD=X`, `mysql -pX` | le mot de passe |
| `token` | `--token X`, `TOKEN=X` | le jeton |
| `api_key` | `--api-key X`, `API_KEY=X` | la clé |
| `credential_url` | `scheme://user:motdepasse@hôte` | le mot de passe de l'URL |
| `env_secret` | `AWS_SECRET_ACCESS_KEY=X`, `*_SECRET=X` | la valeur |
| `private_key` | fragment `-----BEGIN ... PRIVATE KEY-----` | commande entière |
| `unknown_sensitive` | mot-clé de `sensitive_keywords` sans structure connue | commande entière |

Les valeurs entre guillemets simples ou doubles sont gérées (`PASSWORD="a b c"`).

### Choix volontaires (limites)

- Le mot de passe attaché `-p<valeur>` n'est traité **que** pour les clients SQL
  reconnus (`mysql`, `mariadb`, `psql`, `mysqldump`), afin d'éviter les faux
  positifs courants (`mkdir -p`, `find -print`...).
- La redaction remplace **toutes** les occurrences d'une valeur détectée dans la
  commande. Cette sur-redaction éventuelle est sans danger : elle ne fait que
  masquer davantage.
- La détection peut manquer un secret à la structure inhabituelle. Dans ce cas,
  enrichissez `sensitive_keywords` (repli `unknown_sensitive`) ou supprimez la
  commande avec `mnemo delete`.

## Utilisation

```bash
# Inventaire (lecture seule)
mnemo secrets scan
mnemo secrets scan --limit 20
mnemo secrets scan --json        # intégration outillée, sans aucune valeur en clair

# Nettoyage
mnemo secrets redact             # dry-run : montre ce qui serait redacté, ne modifie rien
mnemo secrets redact --apply     # applique (demande confirmation si interactif)
mnemo secrets redact --apply --yes  # applique sans question (scripts)
```

Le drapeau `--backup` rend explicite la sauvegarde, qui est de toute façon
**toujours** effectuée avec `--apply`.

## Garanties de sûreté

1. **Dry-run par défaut** : sans `--apply`, la base n'est jamais modifiée.
2. **Sauvegarde obligatoire** : avec `--apply`, une sauvegarde complète
   (`.tar.gz`) est créée avant toute écriture. Si la sauvegarde échoue, la
   redaction est annulée et rien n'est modifié.
3. **Confirmation** : en mode interactif, `--apply` demande confirmation. En mode
   non interactif (script, pipe), l'application est refusée sans `--yes`.
4. **Modification minimale** : seule la colonne `command` est réécrite, en
   requête paramétrée, dans une transaction unique. Horodatage, dossier de
   travail, code de sortie, contexte Git et `session_id` sont conservés.
5. **Idempotence** : une commande déjà redactée (`[REDACTED]` /
   `[REDACTED COMMAND]`) est ignorée par les passes suivantes.

## Restauration

La redaction réécrit l'historique. Pour revenir en arrière, restaurez la
sauvegarde créée avant l'opération :

```bash
mnemo restore <archive.tar.gz>
```

Le chemin exact de la sauvegarde est affiché à la fin de chaque redaction
appliquée.

## Voir aussi

- [docs/THREAT_MODEL.md](THREAT_MODEL.md) — menace M8 (fuite de commandes
  sensibles).
- Option de configuration `sensitive_keywords` (filtrage à l'enregistrement et
  repli de détection).
