# C12 — couche système macOS

Statut : BLOQUÉ : pas de Mac.

Environnement de cette session : Linux x86-64, aucun hôte macOS mis à disposition.
Aucun test d'exécution macOS, ni Intel ni Apple Silicon, n'est déclaré réussi.

Défauts confirmés par lecture de la base a824c76 :

- compiler/nxc_sema/src/load.rs sélectionne SYS_LINUX pour Os::Linux | Os::MacOs.
- lib/std/sys/linux.nx emploie les numéros et structures système Linux.
- Le générateur partage encore l'entrée Linux/Mac et son assemblage ELF `.type`.
- link_macos produit une cible Mach-O et des stubs libSystem ; cela ne rend pas les
  appels système Linux corrects ni n'établit la compatibilité du runtime.

La compilation de l'IR avec des attributs macOS (C1) ne vaut donc pas exécution macOS.
La condition de fin C12 « tests sur un Mac, ou BLOQUÉ : pas de Mac écrit » est consignée
ici, sans prétendre corriger la cible. C2 macOS reste dépendant de ce travail.

Étape proposée à Claude : choisir soit une vraie couche Darwin/libSystem avec son
entrée et ses structures signal, soit une exclusion explicite des cibles macOS du CLI
et de la documentation. Ce second choix touche les documents qu'il doit relire ; les
fichiers compiler/nxc_sema et docs/spec sont aussi réservés selon la répartition #45.
Aucune modification concurrente de ces fichiers n'est faite par cette PR.

Pour valider une future couche : tests/run O0/O2, entrées-sorties Unicode, allocations,
arguments/environnement, temps/aléatoire, FFI et défaillances de pile sur les deux
architectures ; documenter précisément les machines et versions. Avant cela, aucune
promotion de macOS comme plateforme fonctionnelle ou garantie.

Vérification actuelle : lecture des chemins ci-dessus et identification de l'hôte ;
pas de test matériel. git diff --check réussi.
