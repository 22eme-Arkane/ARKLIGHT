# Session de capture Astera — mode d'emploi

**But :** enregistrer, sur une vraie installation Astera, ce que l'application échange avec le « pont Bluetooth ». Ce pont est soit une **AsteraBox**, soit une **lampe BTB** (Titan, Helios, NYX, AX9… qui ont un pont intégré). Avec ces enregistrements, le simulateur pourra se faire passer pour ce pont, et ton téléphone pilotera les projecteurs virtuels avec la vraie application.

**Durée :** ~15 min de préparation la veille, ~45 min sur place.

> Pourquoi c'est nécessaire : le protocole entre l'appli et le pont n'est publié nulle part. Aucune documentation ni aucun projet libre n'existe. On ne peut l'imiter qu'en observant de vrais échanges.

---

## 0. Ce qu'il te faut

- Le **téléphone Android** avec **AsteraNext** (ou l'ancienne AsteraApp) installée et à jour. C'est lui qui enregistre : l'iPhone ne sait pas le faire.
- Son **câble USB**, pour récupérer les fichiers sur le PC.
- Sur place :
  - le pont utilisé d'habitude (AsteraBox **ART7**, AsteraBox **ART7-WIFI**, ou une lampe BTB) ;
  - **2 ou 3 lampes de modèles différents** si possible (un tube Titan/Helios/Hyperion, un PAR AX3/AX5/AX9, une NYX…), batteries chargées.
- De quoi noter l'heure de chaque étape. Les **captures d'écran** du téléphone suffisent, elles sont horodatées.

---

## 1. La veille : préparer le téléphone (10 min)

1. **Activer les options pour les développeurs.** Paramètres → À propos du téléphone (→ Informations sur le logiciel) → touche **7 fois** « Numéro de build ».
2. **Ouvrir les options pour les développeurs** (Paramètres → Système → Options pour les développeurs ; chez Samsung : Paramètres → Options de développement), puis :
   - **« Activer le journal HCI Bluetooth »** (ou « Journal de surveillance HCI Bluetooth ») → **Activé**. Si on te propose *Filtré* ou *Complet*, choisis **Complet**.
   - **« Débogage USB »** → Activé.
3. **Éteins puis rallume le Bluetooth.** Le journal ne démarre qu'à ce moment-là. Sur certains modèles, il faut redémarrer le téléphone.
4. **Installe « nRF Connect for Mobile »** (Nordic Semiconductor, gratuit, Play Store). Il sert à relever l'identité Bluetooth du pont.
5. Si ton pont est une **ART7-WIFI** (c'est écrit au dos) : installe aussi **« PCAPdroid »** (gratuit, Play Store), pour enregistrer le trafic Wi-Fi de l'appli.
6. **Test au PC.**
   1. Branche le téléphone.
   2. Dans le dossier du simulateur (`ARKLIGHT`), ouvre un terminal (clic droit → « Ouvrir dans le terminal ») et tape :
      ```
      adb devices
      ```
   3. Le téléphone demande d'autoriser ce PC : accepte, en cochant « Toujours autoriser ». La commande doit afficher une ligne se terminant par `device`.

---

## 2. Sur place : l'identité du pont (5 min, appli fermée)

1. **Photographie l'étiquette du pont**, au dos de la box ou sous la lampe BTB : modèle (ART7 / ART7-WIFI / modèle de lampe), numéro de série et **PIN Bluetooth**.
   - Ce PIN est calculé à partir du numéro de série. Il faudra le reproduire pour que l'appli accepte la fausse box.
   - Garde ces photos pour toi.
2. Ouvre **nRF Connect** → onglet **SCANNER** → **SCAN**.
   1. Repère le pont. Son nom contient souvent « Astera », « ART7 » ou le nom de la lampe ; sinon, c'est le signal le plus fort tout près de lui.
   2. Touche la ligne pour la déplier → **capture d'écran**.
   3. Touche **RAW** (octets bruts) → **capture d'écran**.
3. Touche **CONNECT**.
   1. La liste des services s'affiche : déplie chaque service « Unknown » → **capture d'écran** de chacun (UUID et propriétés).
   2. Si un **code PIN** est demandé, fais une capture d'écran de la demande, puis annule.
   3. **DISCONNECT**, puis ferme nRF Connect.

---

## 3. Sur place : la capture principale (20–30 min)

**Règles d'or :**
- **Une action à la fois**, puis **5 secondes sans rien toucher**. L'outil d'analyse découpe les échanges sur ces silences.
- **Une capture d'écran du téléphone après chaque action.** Grâce à l'heure, on retrouvera quel message correspond à quelle action.
- Si une étape n'existe pas dans l'appli, passe-la.

*Optionnel :* juste avant de commencer, si tu sais refaire l'appairage avec le PIN, « oublie » le pont dans les réglages Bluetooth du téléphone. On verra ainsi l'appairage complet dans la capture.

### A. Connexion
- [ ] A1. Bluetooth activé, **lance AsteraNext**.
- [ ] A2. **Connecte-toi au pont.** Saisis le PIN si demandé, puis attends que les lampes apparaissent (~10 s).
- [ ] A3. Si l'appli propose **« Pair with Lights »** (BlueMode) : maintiens le bouton d'une lampe ~3 s jusqu'au clignotement bleu, puis touche « Pair with Lights ».

### B. Commandes simples (d'abord sur UNE lampe)
- [ ] B1. Allumer / éteindre la lampe.
- [ ] B2. Intensité : 100 % → 50 % → 10 % → 0 %, en 4 actions séparées.
- [ ] B3. Kelvin : 3200 K → 5600 K → 10000 K.
- [ ] B4. Teinte : plein **+vert** → 0 → plein **magenta**.
- [ ] B5. Couleurs pures : **rouge**, **vert**, **bleu** (RGB ou HSI).
- [ ] B6. Saturation 50 %.
- [ ] B7. Un **gel** (ex. Lee 201) et une **source** (ex. Tungsten, HMI), s'ils existent.
- [ ] B8. Refais B2 et B5 avec **toutes les lampes** sélectionnées (ou un groupe).

### C. Effets / programmes
- [ ] C1. Lance un **stroboscope**, puis change sa vitesse 2 fois.
- [ ] C2. Lance un effet **chenillard / pixel** sur un tube, puis change la couleur de l'effet.
- [ ] C3. Arrête les effets et reviens à une couleur fixe.

### D. Réglages des lampes
- [ ] D1. **Renomme** une lampe (ex. « TEST1 »).
- [ ] D2. **Identifie** une lampe (faire clignoter).
- [ ] D3. Change l'**adresse DMX** (ex. 10), puis le **mode / profil DMX**.
- [ ] D4. Ouvre la page **infos** d'une lampe (batterie, firmware…) et attends 10 s.
- [ ] D5. Change la **durée d'autonomie** (runtime), si le menu existe.
- [ ] D6. **Crée un groupe** de 2 lampes, puis règle l'intensité du groupe.

### E. Fin
- [ ] E1. Laisse l'appli ouverte **30 s sans rien toucher**. On verra les messages d'état envoyés tout seuls (batterie…).
- [ ] E2. **Déconnecte-toi** du pont, puis ferme l'appli.

### Si le pont est une ART7-WIFI : 2ᵉ capture en Wi-Fi
1. Ouvre **PCAPdroid**, choisis **AsteraNext** comme application cible, puis le mode de sauvegarde **« Fichier PCAP »**.
2. Démarre la capture.
3. Refais **A2, B2, B5, D2 et E2**.
4. Arrête la capture.

---

## 4. Récupérer et analyser (au PC, 5 min)

1. Branche le téléphone et ouvre un terminal dans le dossier du simulateur (`ARKLIGHT`).
2. Récupère le rapport du téléphone (1 à 3 min ; le téléphone peut demander une confirmation) :
   ```
   adb bugreport captures\session-astera-1.zip
   ```
3. Lance l'analyse :
   ```
   node tools\btsnoop\analyze.mjs captures\session-astera-1.zip --filtre astera
   ```
   Elle produit `captures\session-astera-1.rapport.md`, lisible, et `captures\session-astera-1.json`, pour l'émulateur.
4. Copie les captures d'écran du téléphone dans `captures\ecrans-session-1\`, et les photos de l'étiquette dans `captures\`.
5. Si tu as une capture PCAPdroid, copie-la aussi dans `captures\`.
6. Ensuite, demande-moi d'exploiter la session : je lis le rapport, je fais correspondre chaque rafale avec tes captures d'écran, et je complète l'émulateur `server/ble/emulators/astera-box.js`.

**Si l'outil dit « Aucun journal btsnoop dans cette archive » :**
- Vérifie que le journal HCI était bien **Activé (Complet)**, et que tu as éteint/rallumé le Bluetooth (ou redémarré) **avant** la session.
- Certains téléphones ne mettent le journal dans le rapport qu'après un redémarrage avec l'option activée.

---

## 5. Optionnel : récupérer l'application pour analyse

L'appli AsteraNext est écrite en C# (.NET MAUI). Son code contient le protocole complet, y compris un éventuel chiffrement et le calcul du PIN. L'analyser est donc le moyen le plus sûr de compléter les captures.

Le droit européen (directive 2009/24/CE, art. 6) autorise, sous conditions, cette analyse à des fins d'**interopérabilité** : sur une copie obtenue légitimement, limitée à ce qui est nécessaire, sans rien redistribuer.

```
adb shell pm path com.asteraled.asteranext
```
La commande affiche une ou plusieurs lignes `package:/data/app/…/base.apk`. Copie le chemin de **base.apk** (sans « package: ») :
```
adb pull <chemin copié> captures\asteranext-base.apk
```

---

## Confidentialité

- Le journal HCI contient **tout le Bluetooth du téléphone** pendant la période (écouteurs, montre…).
- Le rapport `adb bugreport` contient beaucoup d'informations sur le téléphone. L'outil n'en lit que le journal Bluetooth.
- Tout reste dans `captures\`, qui est exclu du dépôt git. Après analyse, tu peux supprimer le .zip.
- Après la session, tu peux désactiver le journal HCI et le débogage USB.
