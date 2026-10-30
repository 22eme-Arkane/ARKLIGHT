# ARKLIGHT en Rust : plan de migration

Décidé le 27/10/2026. La version JavaScript (Electron, three.js) reste celle qu'on installe et
qu'on publie jusqu'à ce que la version Rust fasse au moins aussi bien ; les deux vivent dans ce
dépôt pendant la migration (`rust/`), pour comparer leurs résultats sur les mêmes projets.

## Décisions

| Sujet | Choix | Pourquoi |
|---|---|---|
| Version PC | Rust natif : moteur 3D **wgpu** (DirectX 12 / Vulkan / Metal), interface **Slint** | Maîtrise complète du rendu (ombres, faisceaux dans la brume, halo) et de la mémoire ; Slint donne une interface soignée et tourne aussi sur Android et iOS |
| Vue 3D dans l'interface | Le moteur dessine dans une texture wgpu affichée par Slint (`unstable-wgpu-30`), sur la même carte graphique | Pas de copie d'image, pas de fenêtre superposée |
| Tablette et téléphone | **Nouvelle app** ARKLIGHT mobile : plan de feu 2D, patch DMX, liaison Blackout, pilotage des Astera et des autres projecteurs, en remplacement de l'AsteraApp | Pas de 3D ; l'app ARK-LIGHT (Flutter, référence pédagogique) reste à part, inchangée |
| Partage PC / mobile | Un **cœur Rust commun** : bibliothèque, moteur DMX et couleurs, format de projet, plan 2D, adressage, MVR, réseau (Art-Net, sACN, RDM), Bluetooth | Une seule vérité pour les données et les protocoles |
| Format des projets | Le même `.json` (format 2) : un projet s'ouvre dans les deux versions | Aucune perte pendant la migration ; les champs inconnus sont gardés tels quels |
| Licence | ARKLIGHT passe sous Apache-2.0 (version 1.0) ; Slint est utilisé sous sa licence Royalty-free | Attribution « Made with Slint » dans la fenêtre « À propos » (AboutSlint) et le README |

## Organisation (`rust/`)

```
rust/
  Cargo.toml                  espace de travail
  crates/
    arklight-core/            cœur commun PC + mobile (sans interface)
      data/library.json       bibliothèque exportée de shared/ (tools/rust/export-library.mjs)
      src/project.rs          lecture / écriture des projets (format 2), valeurs par défaut
      src/library.rs          projecteurs, décor, grip, pieds
      src/color.rs            Kelvin, teinte → couleur
      src/engine/             moteur lumière : DMX, effets, gélatines, console, Astera, sorties
      tests/                  essais de parité (tools/rust/export-*.mjs)
      src/document.rs         projet ouvert : modifications, annuler / rétablir
      src/addressing.rs       adressage DMX automatique (conflits, à la suite)
    arklight-net/             Art-Net, sACN, fusion des sources, RDM (PC et mobile)
    arklight-plot/            plan de feu 2D, liste du matériel, patch, MVR (PC et mobile)
    arklight-onboard/         menus embarqués des projecteurs (écrans, touches, molettes)
    arklight-midi/            contrôleurs MIDI (WinMM), préréglages, apprentissage
    arklight-ai/              assistant IA : outils, AIODOS, Ollama, API en ligne (HTTPS rustls)
    arklight-ble/             pont Bluetooth LE (le PC joue un appareil : fausse AsteraBox)
    arklight-render/          moteur 3D wgpu (PC seulement)
    arklight-desktop/         application PC (Slint)
  installer/arklight.iss      installeur Windows (Inno Setup, style d'AIODOS aux couleurs d'ARKLIGHT)
  installer/legal/            charte et conditions d'utilisation (fr, en)
```

Plus tard : `arklight-mobile` (l'app tablette et téléphone).

## Étapes

Chaque étape se termine par une version qui compile, se lance et se compare à la version
JavaScript sur les projets d'exemple (nuit, clip).

1. **Socle** — cœur commun (projet, bibliothèque), fenêtre Slint, vue 3D wgpu : sol, cyclo,
   projecteurs, décor en volumes simples, lumière des projecteurs, caméra (orbite, zoom,
   décalage), ouvrir un projet, menus FR/EN. *(commencée le 27/10/2026)*
2. **Moteur lumière** — portage de `shared/engine` (couleurs, DMX, console, Astera) avec des
   essais de parité : mêmes entrées, mêmes sorties qu'en JavaScript. *(faite le 27/10/2026 :
   4 500 cas comparés — les 746 modes DMX distincts, la console, 96 scénarios image par image,
   les projets d'exemple ; reste pour plus tard : accessoires et gélatines posées sur les
   projecteurs, zoom, menus embarqués et identification RDM)*
3. **Rendu** — modèles des projecteurs, du décor (98 objets), du grip et des pieds ; ombres,
   faisceaux dans la brume, halo des sources, exposition en IL ; objectif 60 images/s sur le
   projet « clip ».
   - 3a *(fait)* : modèles des projecteurs (pixels émetteurs, spots, sources ponctuelles), pieds,
     matériaux comme three.js, faisceaux dans la brume, halo des sources, image HDR ;
   - 3b *(fait)* : accessoires (boîtes à lumière, dômes, grilles, gélatines, zoom : essais de
     parité), grip et ses toiles, ombres nettes, atténuation, faisceaux coupés, rebonds ;
   - 3c *(fait)* : modèles du décor (personnages et leurs poses, animaux, véhicules, mobilier,
     architecture, extérieur, accessoires et matériel de plateau : atelier `props/kit.rs`) ;
   - 3d *(fait)* : comparaison avec la version JavaScript sur les exemples (même caméra, mêmes
     intensités) ; environ 1 ms par image sur les deux exemples (RTX 4070).
4. **Interface** — bibliothèque, liste Scène, inspecteur, console, moniteur DMX, menus des
   projecteurs (panneaux embarqués), raccourcis, annuler / rétablir, écran des projets,
   séquences et plans. *(faite le 27/10/2026 : outils Déplacer (W) / Tourner (E, pan et tilt par
   5°) dans la vue 3D ; écran des projets avec vignettes, éditeur de raccourcis, étiquettes 3D (N) ;
   dictée ORAKLE dans les champs de texte (micro, info-bulle des raccourcis) ; menus
   Sélection et Patch, clic droit, groupes, viser / poser au sol / cadrer, numérotation,
   ré-adressage (3 520 plans identiques au JavaScript), faces avant des 30 modèles de marque et
   des Astera (46 scénarios, 33 000 comparaisons identiques))*
   *Refonte du 27/10/2026, sur le modèle de la version JavaScript : fenêtre composée comme
   `web/index.html` (barre du haut avec logo, menus intégrés, nom du projet, séquence › plan,
   voyants et don ; barre d'outils à icônes de la vue ; panneau de gauche bibliothèque + scène ;
   inspecteur ; tiroir du bas), un fichier Slint par zone (`ui/header`, `viewbar`, `left`,
   `view`, `inspector`, `drawer`, `menus`, `common`, `theme`) ; thème et éléments de base repris
   de `web/css/app.css` (`theme.slint` : couleurs, boutons, curseurs, listes, cases, faders,
   bulles d'aide ; icônes de `web/js/ui/icons.js` et de la bibliothèque) ; bibliothèque à onglets
   (Lumières, Pieds, Grip, Décor), tris, marques repliables, glisser dans la vue
   (`src/library_panel.rs`) ; liste Scène avec groupes, patch et conflits, recherche et tris
   (`src/scene_panel.rs`) ; inspecteur en sections ; vues face / dessus / côté (1 à 4),
   aimant (M), vue en plein écran (Maj+F11) ; barre d'axes X / Y / Z pour déplacer (comme
   TransformControls), glisser linéaire sans envol vers l'horizon, pas de 10 cm.*
   *Parité avec la version JavaScript le 29/10/2026 (inventaire `docs/rust/ECARTS-JS.md`, presque
   vide) : inspecteur et faces avant complets, anneaux de rotation, « Voir par la caméra »,
   surbrillance, moniteur DMX, journal et réseau, télécommande infrarouge, menus du clic droit,
   fenêtre « Séquences et plans… », impression plan + liste + patch, volumes du plan et image de
   fond au sol, fenêtres Quitter / Enregistrer / Faire un don / À propos dans l'application,
   panneaux redimensionnables, plein écran, une seule fenêtre, raccourcis (clavier AZERTY dessiné,
   touches dans les menus et les bulles), préréglages MIDI, photo jointe à l'assistant.*
5. **Plan 2D et exports** — éditeur du plan de feu (dans le cœur commun), PNG / SVG / PDF,
   CSV, MVR. *(faite : vue du plan (P), outils de dessin (rectangle, ellipse, forme, main levée,
   ligne, flèche, texte, mesure ; 275 gestes identiques au JavaScript), image de fond et son calage
   par deux points ; exports SVG, PNG, CSV et MVR identiques octet par octet au JavaScript, PDF A4)*
6. **Réseau et appareils** — Art-Net, sACN, RDM, MIDI, fausse AsteraBox (Bluetooth).
   *(faite : RDM identique octet par octet sur 878 requêtes, MIDI sur 239 étapes ; pont Bluetooth
   LE en Rust (crate `windows`, onglet Bluetooth, profil de test « écho ») ; la fausse AsteraBox
   attend les captures du protocole)*
7. **Assistant IA** — AIODOS (client Rust de sa passerelle), Ollama, API en ligne, voix
   (whisper.cpp). *(faite sauf la voix : outils identiques au JavaScript, AIODOS, Ollama,
   Anthropic, OpenAI, Mistral, Gemini, clés chiffrées DPAPI ; voix par ORAKLE, l'application de
   dictée d'ARKANE, grâce à son API locale (ORAKLE 0.1.4) : F8 maintenu pour parler à l'assistant,
   micro dans chaque champ de texte)*
8. **Installeur et mises à jour** — puis la version Rust remplace la version JavaScript.
   *(installeur construit : `node tools/rust/build-installer.mjs` → `rust/target/installer/` ;
   Inno Setup en style sombre comme celui d'AIODOS, aux couleurs d'ARKLIGHT : accueil avec le
   logo et la présentation, charte et conditions d'utilisation à accepter, AIODOS en option ;
   programme autonome (bibliothèque C intégrée) ; vérification des mises à jour sur GitHub.
   Remplacement de la version JavaScript : à décider)*

L'app mobile démarre quand le cœur commun a le moteur DMX, le plan 2D et le réseau
(étapes 2, 5 et 6). Le pilotage direct des Astera attend les captures Bluetooth
(protocole AsteraBox non publié).

## Construire et lancer

```
cd rust
cargo run --release -p arklight-desktop                      projet vide
cargo run --release -p arklight-desktop -- chemin\projet.json   un projet
cargo test                                                   essais
```

Données des essais de parité (à refaire quand le moteur JavaScript change) :

```
node tools/rust/export-library.mjs        bibliothèque
node tools/rust/export-test-data.mjs      couleurs, projets d'exemple
node tools/rust/export-engine-data.mjs    moteur lumière
node tools/rust/export-addressing-data.mjs adressage DMX
node tools/rust/export-plot-data.mjs      plan 2D, CSV, MVR
node tools/rust/export-plot-editor-data.mjs outils de dessin du plan
node tools/rust/export-rdm-data.mjs       RDM
node tools/rust/export-midi-data.mjs      MIDI
node tools/rust/export-onboard-data.mjs   menus embarqués
node tools/rust/export-ai-data.mjs        outils de l'assistant
node tools/rust/export-i18n.mjs           dictionnaire anglais (textes des modules)
```

Installeur (local, rien n'est publié) :

```
node tools/rust/build-installer.mjs
```

Prérequis : Rust (rustup) et les Build Tools de Visual Studio (C++).
