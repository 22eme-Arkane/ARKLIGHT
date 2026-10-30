# Écarts entre la version JavaScript et la version Rust

Inventaire du 29/10/2026 (lecture du code des deux versions). Le but : retrouver dans la version
Rust tout ce que faisait la version JavaScript. Fait le même jour : inspecteur complet, faces avant
(claviers Astera, faces des marques), écran des projets, fenêtres Quitter / Enregistrer / Faire un
don, poignées des panneaux (Ctrl+B, Ctrl+Alt+B, Ctrl+J), Couper / Copier / Coller, pose des
projecteurs ajoutés (hauteur selon la forme, tournés vers le personnage, « + » en arc), touches
1 à 0 sur clavier AZERTY. Puis : menu « Outils » (1), « Voir par la caméra » (3), « Surbrillance » (4),
onglets « Journal » et « Réseau » (5), console complète (6), moniteur DMX (7), télécommande
infrarouge ARC1 / ARC3 (8), adresse IP du PC dans la barre du haut (25, en partie). Puis :
anneaux de rotation X / Y / Z (9), Fichier : « Revenir à la version enregistrée », « Renommer le
projet » (F2), « Dossier des projets… » (19), étiquettes 3D noms / adresses DMX gardées dans le
projet, estompées si éteint (20), menus du clic droit : pied et accessoires des projecteurs, menu
des objets (toile, pieds, pose, « Voir par cette caméra », « Tourner de 90° ») (21, sauf la liste
Scène), moniteur DMX qui se place sur le projecteur choisi, barre du haut qui se resserre dans une
fenêtre étroite, plus aucune boîte de Windows (MIDI « Tout effacer » dans la fenêtre d'ARKLIGHT). Puis :
plan 2D : pointillés, épaisseur, taille du texte, hauteur 3D (14), fenêtre « Fond et cartouche »
(couleur du fond, image de fond : largeur, rotation, opacité, centrer, retirer ; production, chef
électro) (15, sauf l'image au sol en 3D), panneaux redimensionnables (bords à tirer), tailles et
replis gardés (17), plein écran F11 / Échap (18, sauf F10), menus du clic droit de la liste Scène
(objets, CRMX d'un groupe, pied et accessoires, « Sélectionner tous les… ») (21), aide, i/s et halo
gardés (26, en partie). Puis : fenêtre « Séquences et plans… » (vignettes des plans, ouvrir,
renommer, dupliquer, déplacer, supprimer, nouveau plan / nouvelle séquence) (11). « Imprimer / PDF (plan + liste
+ patch) » (Ctrl+P) : page HTML ouverte dans le navigateur, qui lance l'impression (12). Formes du
plan en volume dans la vue 3D et image de fond posée au sol (passe « decal » du moteur) (13, 15). « À propos »
complet, « Rechercher une mise à jour », bandeau « Télécharger → » / « Nouveautés » (24). F10 (menus
au clavier) (18), voyants Art-Net / sACN à états (données, à l'écoute, erreur, bulles, clic : onglet
Réseau ; MIDI et AsteraBox cliquables) (25), « Annulé : … » / « Rétabli : … », × de l'aide, une
seule fenêtre (un second lancement passe son projet à la première, 127.0.0.1) (26). MIDI : canal
du préréglage, « Enregistrer comme préréglage… », « Exporter… », « Importer… », supprimer un
préréglage enregistré (22). Raccourcis :
les 13 commandes qui manquaient (exports du plan, mise à jour, code source, signaler un problème…),
touches par défaut de la version JavaScript (F1 / Ctrl+Alt+K, Ctrl+Q, Ctrl+I, Fin, Ctrl+Maj+P,
Ctrl+Maj+O), recherche dans la fenêtre des raccourcis (16, en partie). Assistant IA :
photo jointe (réduite à 1600 px, « Aussi en fond du plan 2D »), « Réinterpréter un plan dessiné… »
(Outils), exemples, adresse du serveur (Ollama, LM Studio…) (23). Bulles d'aide
de la barre de la vue : touches lues dans les raccourcis choisis (16, en partie). Clavier AZERTY
dessiné dans la fenêtre des raccourcis (touches prises, Ctrl / Alt / Maj, clic : ses commandes).

Reste à faire, par ordre de priorité (JS = `web/js/…`) :

| # | Fonction | Version JavaScript | Effort |
|---|---|---|---|
| 16 | « Changer le raccourci » au clic droit d'un élément de menu : les menus de Slint n'ont pas de clic droit (passer par Édition › Raccourcis clavier…) | `commands.js`, `shortcuts.js` | moyen |
| 27 | Ouvrir l'interface depuis un autre appareil (serveur local) — sans doute remplacé par l'app mobile | `server/` | — |
