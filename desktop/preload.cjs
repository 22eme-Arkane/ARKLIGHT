// Pont entre la page et l'application de bureau : fermeture avec confirmation, liens externes.
const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('arkDesktop', {
  /** La fenêtre va se fermer : la page affiche sa confirmation (enregistrer, soutenir…). */
  onRequestQuit: (cb) => ipcRenderer.on('ark:request-quit', () => cb()),
  /** La page a bien pris en charge la fermeture. */
  ackQuit: () => ipcRenderer.send('ark:quit-ack'),
  /** Fermeture confirmée. */
  quit: () => ipcRenderer.send('ark:quit'),
  /** Ouvre une adresse https dans le navigateur. */
  openExternal: (url) => ipcRenderer.send('ark:open-external', url),
  /** Projets : choisir le dossier des projets, ouvrir un fichier, enregistrer sous (fenêtres de Windows). */
  chooseProjectsFolder: () => ipcRenderer.invoke('ark:projects-folder'),
  openProjectFile: () => ipcRenderer.invoke('ark:project-open'),
  saveProjectAs: (thumbnail) => ipcRenderer.invoke('ark:project-save-as', thumbnail),
  /** Montre le fichier d'un projet dans l'Explorateur. */
  revealProject: (path) => ipcRenderer.invoke('ark:project-reveal', path),
  /** Langue de l'interface (titres des fenêtres de Windows). */
  setLanguage: (lang) => ipcRenderer.send('ark:language', lang),
  /** Langues choisies dans Windows, par ordre de préférence (langue « Automatique »). */
  systemLanguages: (process.argv.find((a) => a.startsWith('--ark-system-languages=')) || '').split('=')[1]?.split(',').filter(Boolean) || [],
  /** Ouvre le dossier des projets dans l'Explorateur. */
  showProjectsFolder: () => ipcRenderer.invoke('ark:projects-folder-show'),
});
