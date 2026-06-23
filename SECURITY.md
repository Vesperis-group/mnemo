# Politique de sécurité

Merci d'aider à garder `mnemo` sûr. `mnemo` est un outil **local-first** (un
binaire Rust, base SQLite locale, aucun service réseau à l'usage) ; les rapports
de vulnérabilité restent néanmoins les bienvenus.

## Versions supportées

Les correctifs de sécurité ciblent la **dernière version publiée** (la `latest`
des [Releases](https://github.com/Vesperis-group/mnemo/releases)). Mettez à jour
avant de signaler un problème : `mnemo upgrade`.

## Signaler une vulnérabilité

Privilégiez un **signalement privé**, pas une issue publique :

1. **GitHub Security Advisories** (recommandé) : onglet **Security → Report a
   vulnerability** du dépôt
   (`https://github.com/Vesperis-group/mnemo/security/advisories/new`).
   Le rapport reste privé jusqu'à publication coordonnée d'un correctif.
2. **Si les advisories sont indisponibles** : contactez l'organisation
   `Vesperis-group` via son canal de sécurité (page de l'organisation sur
   GitHub), en demandant une prise de contact privée. N'incluez pas de détails
   sensibles dans un canal public.

Merci de **ne pas** ouvrir d'issue publique ni de PR exposant la vulnérabilité
avant qu'un correctif soit disponible.

### Informations utiles dans un rapport

- version de `mnemo` (`mnemo version`) et système (distribution, WSL ou natif) ;
- description de l'impact et du scénario d'exploitation ;
- étapes de reproduction minimales ;
- toute trace ou preuve de concept utile (sans données personnelles réelles).

## Traitement

Les rapports sont traités **au mieux des disponibilités** des mainteneurs : ce
projet n'offre **aucun SLA contractuel** sur les délais de réponse ou de
correction. L'objectif est un accusé de réception et une première évaluation
dans des **délais raisonnables**, puis un correctif et une divulgation
coordonnée lorsque c'est pertinent. Votre contribution sera créditée dans
l'avis de sécurité si vous le souhaitez.

## Périmètre

Sont particulièrement pertinents :

- exécution de code arbitraire, escalade de privilèges, écriture hors des
  chemins XDG attendus ;
- fuite de données sensibles (la base d'historique, les fichiers `600`) ;
- contournement des vérifications d'intégrité (SHA-256) ou de signature
  (cosign / Sigstore) des releases ;
- problèmes de chaîne d'approvisionnement (dépendances, scripts d'installation).

Pour le détail des hypothèses de sécurité, voir
[`docs/THREAT_MODEL.md`](docs/THREAT_MODEL.md).
