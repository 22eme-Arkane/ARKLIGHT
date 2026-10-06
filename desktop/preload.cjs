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
});
