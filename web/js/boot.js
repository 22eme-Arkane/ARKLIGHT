// Démarrage : quand l'écran des projets s'ouvre au lancement, l'interface reste masquée jusqu'à lui
// (sinon on voit d'abord le projet en cours puis l'écran des projets, comme un bug). Script classique,
// chargé dans <head> avant tout affichage ; app.js retire la classe dès que l'écran est prêt.
(function () {
  try {
    if (new URLSearchParams(location.search).has('smoke')) return; // test automatique
    const reload = sessionStorage.getItem('arklight.langReload');
    const startup = localStorage.getItem('arklight.projects.startup') !== '0';
    if (reload === 'projects' || (!reload && startup)) {
      document.documentElement.classList.add('boot-hub');
      // filet de sécurité : l'interface réapparaît quoi qu'il arrive
      setTimeout(() => document.documentElement.classList.remove('boot-hub'), 6000);
    }
  } catch {
    /* stockage indisponible : démarrage normal */
  }
})();
