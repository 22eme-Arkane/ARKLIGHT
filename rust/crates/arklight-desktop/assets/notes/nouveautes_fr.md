# Nouveautés d'ARKLIGHT

<!-- Une section « ## X.Y.Z · AAAA-MM-JJ » par version, la plus récente en tête. À CHAQUE
     nouvelle version : écrire l'entrée ici ET dans whats_new_en.md. -->

## 1.0.0 · 2026-10-30

**ARKLIGHT 1.0 : entièrement réécrit pour être beaucoup plus rapide, avec tout ce que faisait
la version précédente — et bien plus.**

- **Plus rapide.** Le moteur 3D, la lumière, le réseau et l'interface ont été réécrits : la
  scène s'affiche et réagit sans attendre, même chargée de projecteurs.
- **Les menus des projecteurs dans l'inspecteur.** La face avant de chaque projecteur (claviers
  Astera, ARRI, Aputure, Kino Flo, Hive…) se règle directement, touches et molettes comprises.
  Viennent ensuite ses réglages, son patch DMX, sa position et son état simulé.
- **Déplacer et pivoter à la souris.** Des poignées X, Y, Z pour déplacer, des anneaux pour
  tourner, par pas de 10 cm et de 5° (aimant), sur tout ce qui est choisi.
- **Voir par la caméra** (C), **surbrillance** pour tout voir dans le noir (H), **télécommande
  infrarouge** des Astera (R).
- **Le plan 2D va plus loin.** Les formes dessinées prennent une hauteur et deviennent des
  volumes dans la 3D ; l'image de fond peut se poser au sol ; un bouton « Affichage » montre
  ou masque les noms, les adresses DMX, la grille… ; « Fond et cartouche » règle la couleur,
  l'image de fond et le cartouche ; « Imprimer / PDF » sort le plan, la liste du matériel et le
  patch d'un coup (Ctrl+P).
- **Un plan dessiné sur papier devient un plan de feu.** « Plan dessiné… » : prends la photo du
  plan d'un chef électro, l'assistant le lit et place les projecteurs, le grip, le décor, les
  personnages, la caméra et les formes (une maison, une estrade…) sur le plan et dans la 3D.
- **Toute la gamme de toiles sur les cadres** : soies 1/4, 1/2, full et China Silk, grids et
  Silent Grid 1/4, 1/2, full, Soft Frost, muslins blanchi et écru, Magic Cloth, Ultrabounce,
  Griffolyn, lamés et soft argent ou or, damier Lisa Marie, noir, tulles lavande, simple, double
  et triple, rangés par famille avec leur perte en diaphragmes. Chacune agit sur la lumière.
- **Toutes les images sont acceptées** : photos HEIC d'iPhone et des téléphones récents (sans
  extension Windows à acheter), JPEG, PNG, WebP, GIF, BMP, TIFF, SVG… et, selon les extensions
  de Windows, AVIF, JPEG XL et les fichiers RAW. La photo est remise droite et convertie toute
  seule.
- **L'assistant IA utilise AIODOS par défaut** : les demandes sont calculées sur cet ordinateur.
  Tout se fait seul : AIODOS est lancé s'il est arrêté, l'accès est demandé (il reste à accepter
  dans AIODOS) et le bon modèle est choisi — un modèle qui voit les images pour un plan dessiné.
  S'il n'est pas encore installé, l'assistant le propose d'un clic.
- **Séquences et plans** : une fenêtre avec les vignettes de chaque plan pour ouvrir, renommer,
  dupliquer, déplacer ou supprimer.
- **La console** pilote plusieurs projecteurs du même modèle, avec plein feu, noir, arc-en-ciel
  et les valeurs DMX brutes ; le **moniteur DMX** montre le nom de chaque projecteur sur ses
  canaux et ses conflits d'adresse ; les onglets **Journal** et **Réseau** suivent ce qui
  arrive.
- **Contrôleur MIDI** : choix du canal, préréglages à enregistrer, exporter et importer.
- **Raccourcis clavier** : chaque menu montre sa touche, un clavier AZERTY dessiné montre les
  touches prises, et tout se change dans Édition › Raccourcis clavier.
- **Plus confortable** : ARKLIGHT s'ouvre à tout l'écran, panneaux redimensionnables (leurs
  tailles sont gardées), plein écran (F11), menus du clic droit complets, une seule fenêtre d'ARKLIGHT à la fois, fenêtres
  Quitter, Enregistrer et Faire un don dans l'application.
- **Licence Apache-2.0** : ARKLIGHT reste libre et gratuit, avec une licence plus simple à
  réutiliser.
- **Mises à jour depuis ARKLIGHT.** Quand une nouvelle version est publiée, un bandeau propose
  de la télécharger et de l'installer, sans passer par le navigateur. Cette fenêtre des
  nouveautés s'ouvre ensuite à la première ouverture de chaque nouvelle version.

## 0.1.0-alpha.1 · 2026-10-06

**Première version publique.**

- Simulateur des projecteurs Astera et de leurs menus embarqués (Titan, Helios, Hyperion,
  AX9, NYX…), dans une scène en 3D avec sol et cyclo.
- Réception de l'Art-Net et du sACN : Luminair, Blackout ou une console pilotent les
  projecteurs virtuels, qui répondent au RDM.
- Autres marques (ARRI, Aputure, Kino Flo, DMG, LiteGear, Hive, Dedolight), tungstène et HMI.
- Grip, pieds, décor et personnages ; plan de feu 2D, liste du matériel, patch DMX, export
  MVR.
- Assistant IA (Ollama, AIODOS ou un service en ligne) et contrôleur MIDI.
