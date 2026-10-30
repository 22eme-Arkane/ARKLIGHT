# ARKLIGHT : version JavaScript d'origine

> Document de la version JavaScript (Node, Electron, three.js), publiée en 0.1.0-alpha. L'application publiée aujourd'hui est la version Rust : voir le [README](../README.md). Ce code reste dans le dépôt : `shared/` est la source de la bibliothèque de la version Rust.

Simulateur d'entraînement aux applications de contrôle lumière utilisées en tournage : **AsteraNext / AsteraApp** d'abord, puis **Blackout** et **Luminair**. Logiciel libre et gratuit (Apache-2.0) de 22eme ARKANE.

> **Version alpha** : des fonctions bougent et cassent encore. Non affilié aux fabricants cités.

**[Télécharger pour Windows](https://github.com/22eme-Arkane/ARKLIGHT/releases)** (installateur `ARKLIGHT-Setup-<version>.exe`, rubrique *Assets* de la dernière version).

- Une scène 3D (studio, cyclo, personnage) où l'on place des projecteurs Astera virtuels.
- Les projecteurs réagissent au **DMX** (Art-Net / sACN) envoyé par une appli ou une console, à une **console de test** intégrée, et bientôt à la **vraie appli Astera** via une fausse AsteraBox Bluetooth.
- Chaque projecteur reproduira ses **menus embarqués** (écran, boutons, voyants) pour s'entraîner comme sur le vrai.

## Démarrer

**Application installée (Windows)** : lancer `ARKLIGHT-Setup-<version>.exe`. L'installateur n'est pas signé : Windows affiche « Windows a protégé votre ordinateur » → *Informations complémentaires* → *Exécuter quand même*. Les scènes, réglages et images de fond sont rangés dans `Documents\ARKLIGHT` ; les clés de l'assistant IA, chiffrées, dans le profil de l'application (`%APPDATA%\arklight`), jamais dans Documents. À la fermeture, le logiciel propose d'enregistrer la scène. Au démarrage, il vérifie s'il existe une version plus récente sur GitHub et l'annonce dans un bandeau. L'installateur (en français ou en anglais selon Windows) propose aussi, en option, d'installer **AIODOS** d'ARKANE, qui relie par Internet les ordinateurs de personnes qui ne se connaissent pas pour partager leur puissance de calcul : une fois installé, il prête par défaut une part de ce PC au réseau public (60 % de sa puissance au plus, jamais sur batterie ; tout se coupe dans ses Réglages). Les demandes d'ARKLIGHT, elles, restent sur ce PC (voir *Assistant IA* ci-dessous).

**Depuis le code** — prérequis : [Node.js](https://nodejs.org) 20 ou plus récent.

- Double-clic sur **`start.bat`** : le simulateur démarre et le navigateur s'ouvre sur http://localhost:8686.
- Ou, dans un terminal, depuis ce dossier :
  ```
  npm install
  npm start
  ```

- Application de bureau sans installer : `npm run desktop`. Fabriquer l'installateur : `npm run dist` (résultat dans `dist\`).

Au premier lancement, Windows peut demander d'autoriser **Node.js** (ou ARKLIGHT) sur le réseau : accepte pour les **réseaux privés**. Sinon, les tablettes ne pourront pas joindre le simulateur.

## Utilisation

| Où | Quoi |
|---|---|
| **Menus** (en haut) | **Fichier** : nouvelle scène, ouvrir, enregistrer (Ctrl+S), enregistrer sous, exports du plan de feu · **Édition** : annuler / rétablir (Ctrl+Z / Ctrl+Y), couper, copier, coller, dupliquer, sélection, groupes, adressage · **Affichage** : vues, plan 2D, noms, décor, panneaux à masquer (Ctrl+B, Ctrl+J), plein écran · **Outils** · **Aide**. Chaque commande montre sa touche |
| **Raccourcis clavier** | *Édition › Raccourcis clavier…* (Ctrl+Alt+K ou F1), comme dans Premiere Pro ou DaVinci Resolve : recherche, arborescence des menus, clavier AZERTY qui montre les touches prises (Ctrl, Alt, Maj), plusieurs touches par commande, « + » pour en ajouter, conflits signalés et annulables |
| **Bibliothèque** (gauche) | Onglets **Lumières** (par marque, et Tungstène & HMI), **Pieds**, **Grip**. On les **glisse** dans la vue 3D (le « + » les ajoute à une place proposée) |
| **Scène** (gauche) | Groupes et projecteurs · Ctrl+clic, Maj+clic : sélection multiple · double-clic sur un groupe : renommer · **clic droit** : actions |
| **Vue 3D** | Clic : sélectionner (Ctrl+clic : plusieurs) · glisser : tourner autour · clic droit : menu · clic droit glissé : décaler · molette : zoom |
| | **W** déplacer, **E** pivoter, **F** cadrer, **1–4** vues, **P** plan 2D, **Ctrl+A** tout, **Ctrl+G** grouper, **Suppr** effacer, **Ctrl+D** dupliquer, **Ctrl+C / Ctrl+V** copier-coller, **Ctrl+Z** annuler |
| | Expo en diaphragmes (IL) : baisse-la face à un 2K tungstène ou un HMI · menu Décor : noms et adresses au-dessus des projecteurs |
| **Grip et lumière** | Drapeau, floppy, toile noire : coupent le faisceau et font une ombre nette · tulles : atténuent · soie, grid cloth : diffusent (lumière douce de l'autre côté) · poly, ultrabounce, réflecteur : renvoient la lumière. Limite : les tubes et ampoules ne font pas encore d'ombre |
| **❤ Faire un don** (en haut) | ARKLIGHT est gratuit : don par PayPal ou cryptomonnaie, comme pour PANDORA. Le logo ouvre « À propos » (version, recherche de mise à jour) |
| **Plan 2D** (bouton de la barre d'outils) | Vue de dessus : déplacer, orienter (poignée ⟳), cadre de sélection ; formes (rectangle, ellipse, point par point, main levée), textes, flèches, mesures ; une forme avec une **hauteur** devient un volume en 3D ; **image de fond** (photo, plan) calée à l'échelle sur deux points, visible aussi au sol en 3D ; export **PNG / SVG / impression-PDF** avec **liste du matériel** et **patch**, CSV |
| **Inspecteur** (droite) | Nom, position, **support** (pied de 1000, junior, wind-up… choisi automatiquement, avec alertes de charge et de hauteur), patch DMX, état simulé · sélection multiple : adressage à la suite, profil commun · grip : toile, hauteur |
| **Console de test** (bas) | Un fader par canal du projecteur sélectionné, pour essayer sans appli |
| **Moniteur DMX** | Les 512 canaux d'un univers, qui les occupe, les **chevauchements** en rouge, et le **ré-adressage automatique** (conflits seulement, tout, sélection, groupe ou univers ; ordre, alignement 1/11/21…, aperçu avant d'appliquer) |
| **Réseau** | Adresse IP à donner aux applis, sources DMX reçues, numérotation Art-Net |
| **Bluetooth** | Démarrage de la fausse AsteraBox, trafic échangé avec le téléphone |
| **Langue** (menu) | **Automatique** (par défaut : la langue de Windows, français ou anglais), Français, English. Aussi sur l'écran des projets |
| **Assistant IA** (bas, Ctrl+Maj+A) | Demandes en français ou en anglais, à la voix (F8) ou au clavier ; il agit dans le plan ouvert (Ctrl+Z annule). *Outils › Réinterpréter un plan dessiné…* : la photo d'un plan fait à la main est redessinée en plan 2D et en 3D. Moteurs : **AIODOS** (la puissance de calcul partagée entre inconnus, par Internet ; les demandes d'ARKLIGHT, elles, sont calculées sur ce PC : « Se connecter à AIODOS », puis un code à 6 chiffres à comparer dans AIODOS), Ollama, serveur local compatible OpenAI, ou API en ligne avec ta clé. L'assistant ne s'utilise que sur l'ordinateur d'ARKLIGHT, pas depuis une tablette du réseau |

Les scènes sont enregistrées dans `Documents\ARKLIGHT\scenes` (application installée) ou `scenes/` (depuis le code). La scène en cours est sauvegardée automatiquement ; un point orange à côté de son nom signale des modifications pas encore enregistrées.

## Projecteurs Astera simulés

| Modèle | Commandes embarquées | DMX |
|---|---|---|
| Titan, Helios, Hyperion (+ versions BTB) | clavier + écran OLED : menu complet (INPUT SELECT, STATIC COLOR, DMX ADDRESS, DMX SETTINGS, UNLINK CRMX, RUNTIME, STANDALONE, AC FAILURE, INFO, RESET), pages couleur (HSI, couleurs fixes, gels LEE), luminosité/autonomie, BlueMode | 143 profils officiels |
| AX2 PixelBar 100 / 50 | même clavier + touche ASTERA (l'adressage auto n'est pas documenté) | 143 profils officiels |
| AX1 PixelTube (+ BTB) | bouton unique : jauge de batterie sur les pixels, BlueMode et ses 13 couleurs, reset (rouge), adressage DMX par secousse | 143 profils officiels |
| AX9 PowerPAR, PixelBrick | même clavier OLED (pont Bluetooth BTB intégré : UNBOND BLUETOOTH dans le menu) | profils 1 pixel officiels (sauf 168–171) |
| HydraPanel | même clavier OLED + TouchSlider (actif sur les pages soleil/palette, DMX ADDRESS, FIXTURE NUMBER) | 143 profils officiels |
| AX3 Lightdrop (+ BTB) | deux boutons en parallèle, la LED unique sert de jauge (couleur) | profils 1 pixel officiels (sauf 168–171) |
| AX10 SpotMax (ancien modèle) | LCD bleu 2 lignes, voyant batterie, 4 touches à double légende (maintien 2 s : COLOR, SPEED, BRIGHT, PROG), menu 2020 (« UNPAIR CRMX ») | profils 1 pixel officiels |
| Pluto (AF80), Leo (AF250) | clavier OLED + TouchSlider, FAN CONTROL, FIXTURE NUMBER, CRMX LINK BY KEY, zoom manuel 15–60° (lux réels) | profils 1 pixel + profils FAN 144–177 |
| NYX Bulb, SolaBulb | une touche (ON/OFF, BlueMode et 13 couleurs, reset) ; zoom 15–50° sur la SolaBulb | profils 1 pixel officiels |
| LunaBulb | aucune touche : BlueMode par la télécommande ARC3 | profils 1 pixel officiels |
| AsteraBox ART7 / WIFI | ⏻ + touche CRMX LINK (lier, délier, choix TX/RX), voyants APP, LINK, DMX, RX, TX, jauge 5 segments | — |
| QuikSpot, QuikPunch, QuikBeam, QuikBeam Connect | clavier OLED + touche ASTERA : inclinomètre (TiltMeter) calculé sur l'orientation 3D ; OUTPUT GAIN (−20 % désactivé) ; FAN CONTROL (Punch, Beam) ; zoom 13–60° (HotBeam sur QuikBeam) avec lux mesurés | profils 1 pixel + profils FAN 144–177 |
| AX5 TriplePAR (+ BTB), AX7 SpotLite | afficheur 2 lignes, voyant batterie, touches à double légende ; AX7 avec les textes du firmware 2018 (UNPAIR CRMX, ARE YOU SURE? NO/YES) | profils 1 pixel officiels |

Le comportement suit le firmware Astera :
- **Sélection d'entrée** : en INPUT SELECT = AUTO, la première source active est verrouillée jusqu'à l'extinction.
- **Liaison CRMX** : un projecteur reçoit le DMX seulement s'il est lié. Pour le lier, on le délie dans son menu, puis on appuie sur la touche LINK d'une AsteraBox placée dans la scène.
- **DMX FAIL** : HOLD, EMERGENCY LIGHT, BLACKOUT ou STANDALONE en cas de perte du signal.
- **Autonomie** : la durée choisie bride la sortie. À 5 h, la valeur par défaut, un tube donne environ 30 % de sa puissance.
- **Programmes autonomes** : 17 programmes, utilisables aussi en mode d'effet DMX.

Les spécifications utilisées (menus, gestes, voyants) viennent des manuels et fiches techniques publics des fabricants. Les notes de recherche qui les résument (`docs/recherche/`, citées dans le code) restent hors du dépôt public. Les points non documentés sont marqués dans le code.

## Autres marques, tungstène et HMI

- **Profils DMX officiels** importés de l'[Open Fixture Library](https://open-fixture-library.org) (licence MIT) : ARRI (SkyPanel, L-Series, Broadcaster), Aputure, Kino Flo, DMG Lumière, LiteGear, Hive, Dedolight LED. Pour en ajouter : compléter la liste de `tools/ofl/import.mjs` puis `node tools/ofl/import.mjs`.
- Leur **sortie lumineuse est estimée** (badge « est. ») tant qu'on n'a pas la fiche photométrique ou une mesure.
- **Tungstène** (ARRI 650 Plus, Mole, Dedolight, open face, PAR64, Molefay) : pilotés par un gradateur (1 canal). La lumière se réchauffe quand on baisse (3200 K → environ 2970 K à 50 %) et le filament suit avec un léger retard.
- **HMI** (ARRI M18, K5600 Joker²) : gradation de 50 à 100 %, montée en régime après l'allumage (comportement approché).
- Nanlux, Nanlite et Creamsource (Vortex) ne sont pas dans les bases libres : à ajouter depuis les fiches des fabricants.

## Connecter Luminair, Blackout ou une console

1. Mets le PC et l'iPad / iPhone sur le **même réseau Wi-Fi**. Idéalement un routeur dédié ; évite les réseaux « invités » qui isolent les appareils.
2. Dans l'appli, choisis **Art-Net** ou **sACN**.
3. Indique comme destination l'**adresse IP du PC** (affichée en haut à droite et dans l'onglet *Réseau*).
   - Luminair peut aussi trouver le simulateur tout seul : il apparaît sous le nom **ARKLIGHT** dans la liste des nodes Art-Net.
4. Patche dans l'appli les mêmes projecteurs, aux mêmes adresses que dans le simulateur.
5. Numérotation des univers : par défaut, **Art-Net 0 = Univers 1** du simulateur (réglable dans l'onglet *Réseau*).

Pour tester sans tablette, `tools/artnet-test.mjs` joue le rôle d'une appli :

```
node tools/artnet-test.mjs poll          # découverte : le simulateur doit répondre
node tools/artnet-test.mjs chase 0 48    # chenillard RGB sur l'univers Art-Net 0
```

## Fausse AsteraBox (Bluetooth) : où on en est

Le protocole entre l'appli Astera et l'AsteraBox (ou une lampe « BTB ») **n'est pas public**. La démarche :

1. **Capturer** de vrais échanges avec le téléphone Android : voir [docs/CAPTURE_BLUETOOTH.md](CAPTURE_BLUETOOTH.md).
2. **Analyser** la capture avec `node tools/btsnoop/analyze.mjs <capture.zip>`.
3. **Compléter l'émulateur** `server/ble/emulators/astera-box.js` à partir de l'analyse.
4. **Publier** la fausse box : onglet *Bluetooth* → « Démarrer le Bluetooth ».
   - Le PC sert d'abord de radio (pont Windows `ble/bridge.py`).
   - Si l'appli exige une annonce Bluetooth que Windows ne sait pas reproduire (nom, données fabricant…), une carte **ESP32** (~10 €) prendra le relais.

Déjà prêt :
- le pont Bluetooth Windows (le PC se publie comme périphérique Bluetooth LE) ;
- le profil *Test (écho)*, pour vérifier la liaison avec nRF Connect ;
- l'analyseur de captures, qui lit directement le `.zip` d'`adb bugreport` et gère le Bluetooth LE comme le port série classique ;
- le scanner `tools/ble/scan.py`, qui relève l'identité et la table GATT d'une vraie box.

Le Bluetooth demande l'environnement Python du projet. Il est créé une fois pour toutes :

```
python -m venv .venv
.venv\Scripts\python.exe -m pip install bleak websockets
```

Il faut aussi que le **Bluetooth soit activé dans Windows** (Paramètres → Bluetooth et appareils).

## Mises à jour et publication

- Le logiciel interroge les *Releases* GitHub du dépôt indiqué dans `package.json` (`arklight.updateRepo`), pré-versions alpha comprises, au démarrage puis toutes les 6 h.
- `npm run dist` met dans l'installateur la page facultative « AIODOS » si un `AIODOS-Setup-<version>.exe` est trouvé dans `../AIODOS/dist` (ou indiqué par `AIODOS_SETUP`) ; `node tools/aiodos/prepare-installer.mjs --without` puis `npx electron-builder --win nsis --publish never` le construit sans. Le client d'AIODOS (`aiodos-client`, Apache-2.0) est une copie épinglée : `vendor/aiodos-client-<version>.tgz`.
- Publier une version : monter le numéro (`version` dans `package.json`, par ex. `0.1.0-alpha.2`), `npm run dist`, puis créer une *Release* GitHub (marquée pré-version) avec le `.exe` :
  ```
  gh release create v0.1.0-alpha.2 dist/ARKLIGHT-Setup-0.1.0-alpha.2.exe --prerelease --title "ARKLIGHT 0.1.0-alpha.2"
  ```

## Organisation du code

```
server/            serveur Node : interface web, Art-Net, sACN, scènes, Bluetooth
  dmx/             réception Art-Net (avec réponse ArtPoll) et sACN, fusion des sources
  ble/             gestion du pont Bluetooth et émulateurs (test, AsteraBox)
shared/            code commun serveur / navigateur
  fixtures/        bibliothèque des projecteurs (Astera, autres marques, tungstène/HMI, pieds)
  engine/          colorimétrie, décodage DMX, calcul de la lumière émise
web/               interface (three.js) : vue 3D, plan de feu 2D (web/js/plot), panneaux
ble/bridge.py      pont Bluetooth LE Windows (WinRT)
desktop/           application de bureau (Electron) : fenêtre, fermeture avec enregistrement
web/js/commands.js commandes (menus et raccourcis), web/js/shortcuts.js : réglage des raccourcis
tools/             artnet-test, analyseur btsnoop, scanner BLE, import Open Fixture Library
docs/              guides
test/              tests (npm test)
```

## Feuille de route

1. ✅ Socle : scène 3D, placement, Art-Net / sACN, console de test, moniteur DMX.
2. ✅ Bibliothèque Astera : tubes, AX1, AX2, HydraPanel et AsteraBox avec leurs vraies données (dimensions, pixels, lux, profils DMX officiels).
3. ✅ Menus embarqués des 30 modèles Astera (tubes, AX1/2/3/5/7/9/10, PixelBrick, HydraPanel, ampoules, Fresnel, série Quik, AsteraBox) + télécommandes ARC1/ARC3. Restent à faire : PowerStation, PrepInlay / PrepCase, PrepBox, NetBox.
4. ⏳ Fausse AsteraBox (Bluetooth) : journée de captures chez un loueur → émulateur → test avec AsteraNext.
5. ✅ Préparation de tournage : sélection multiple, groupes, ré-adressage automatique, pieds réalistes, grip, plan de feu 2D avec export, liste du matériel et patch.
6. ⏳ Bibliothèque multi-marques : ARRI, Aputure, Kino Flo, DMG, LiteGear, Hive, tungstène, HMI faits ; Nanlux, Nanlite, Creamsource, Litepanels, Quasar… à ajouter ; photométrie à vérifier.
7. Blackout et Luminair : découverte RDM des projecteurs virtuels, envoi de l'adressage préparé vers les vrais projecteurs (RDM), scénarios d'exercices.
8. ✅ Barre de menus, raccourcis personnalisables, annuler / rétablir, copier-coller ; le grip agit sur la lumière (ombres, rebonds, diffusion ; pas encore pour les tubes et ampoules).
9. Plus tard : une photo transformée en volume 3D, l'application mobile.

---

Copyright © 2026 22eme ARKANE — logiciel libre sous licence Apache-2.0 (voir [LICENSE](../LICENSE)). Non affilié aux fabricants cités.
