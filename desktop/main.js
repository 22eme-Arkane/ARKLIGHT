// ARKLIGHT — application de bureau (Electron) : démarre le serveur du simulateur et l'affiche
// dans sa propre fenêtre. À la fermeture, la page propose d'enregistrer la scène (et de soutenir
// le projet), comme PANDORA.
import { app, BrowserWindow, dialog, ipcMain, Menu, shell } from 'electron';
import { appendFileSync, existsSync, renameSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { placeKeyFile } from './keys-file.js';

const here = dirname(fileURLToPath(import.meta.url));
const ROOT = join(here, '..');
const isHttps = (url) => /^https:\/\//i.test(String(url));
/** Langues de Windows, par ordre de préférence (« fr-FR », « en-US »…). */
const systemLanguages = () => {
  try {
    const list = app.getPreferredSystemLanguages();
    return list.length ? list : [app.getLocale()];
  } catch {
    return [app.getLocale()];
  }
};
/** Journal du test automatique (ARKLIGHT_SMOKE) : la console d'Electron n'est pas visible sous Windows. */
const trace = (msg) => {
  if (process.env.ARKLIGHT_SMOKE) appendFileSync(`${process.env.ARKLIGHT_SMOKE}.log`, `${new Date().toISOString()} ${msg}
`);
};
process.on('uncaughtException', (e) => trace(`erreur : ${e.stack || e}`));
process.on('unhandledRejection', (e) => trace(`promesse rejetée : ${e?.stack || e}`));

// Test automatique : profil à part (pas de conflit avec l'application installée déjà ouverte)
if (process.env.ARKLIGHT_SMOKE) app.setPath('userData', join(dirname(process.env.ARKLIGHT_SMOKE), 'arklight-smoke-profile'));

if (!process.env.ARKLIGHT_SMOKE && !app.requestSingleInstanceLock()) {
  app.quit();
} else {
  let win = null;
  let quitting = false;
  let server = null;
  let quitTimer = null;

  app.on('second-instance', () => {
    if (!win) return;
    if (win.isMinimized()) win.restore();
    win.focus();
  });

  app.whenReady().then(async () => {
    process.env.ARKLIGHT_DESKTOP = '1';
    // scènes, réglages et images de fond dans Documents, faciles à retrouver et à sauvegarder
    if (!process.env.ARKLIGHT_DATA) {
      const docs = app.getPath('documents');
      const data = join(docs, 'ARKLIGHT');
      const legacy = join(docs, 'ARK-LIGHT PC'); // dossier des premières versions alpha
      if (!existsSync(data) && existsSync(legacy)) {
        try {
          renameSync(legacy, data);
        } catch {
          /* dossier ouvert ailleurs : on le renommera au prochain lancement */
        }
      }
      process.env.ARKLIGHT_DATA = existsSync(data) || !existsSync(legacy) ? data : legacy;
    }
    // clés de l'assistant IA (AIODOS, clés d'API) : dans le profil de l'application, jamais dans
    // Documents (souvent synchronisé par OneDrive) ; déplacées de Documents\ARKLIGHT au premier
    // lancement. Test automatique : profil à part, les vraies clés ne sont pas déplacées.
    if (!process.env.ARKLIGHT_KEYS) {
      const keys = join(app.getPath('userData'), 'ai-keys.json');
      process.env.ARKLIGHT_KEYS = process.env.ARKLIGHT_SMOKE ? keys : placeKeyFile(keys, join(process.env.ARKLIGHT_DATA, 'ai-keys.json'));
    }
    trace('démarrage du serveur');
    server = await import(pathToFileURL(join(ROOT, 'server', 'index.js')).href);
    const port = await server.ready;
    trace(`serveur prêt sur le port ${port}`);
    const origin = `http://localhost:${port}`;

    Menu.setApplicationMenu(null);
    win = new BrowserWindow({
      width: 1600,
      height: 950,
      minWidth: 1100,
      minHeight: 680,
      show: false,
      backgroundColor: '#0c0e11',
      title: 'ARKLIGHT',
      icon: join(ROOT, 'build', 'icon.png'),
      // langues choisies dans Windows (langue « Automatique » de l'interface)
      webPreferences: { preload: join(here, 'preload.cjs'), contextIsolation: true, nodeIntegration: false, additionalArguments: [`--ark-system-languages=${systemLanguages().join(',')}`] },
    });
    win.once('ready-to-show', () => {
      if (process.env.ARKLIGHT_SMOKE) return win.showInactive(); // test automatique : sans voler le focus
      win.maximize();
      win.show();
    });

    // Liens externes (don, mises à jour, GitHub) dans le navigateur ; la fenêtre d'impression du plan reste ici
    win.webContents.setWindowOpenHandler(({ url }) => {
      if (url === 'about:blank' || url.startsWith(origin)) return { action: 'allow', overrideBrowserWindowOptions: { autoHideMenuBar: true } };
      if (isHttps(url)) shell.openExternal(url);
      return { action: 'deny' };
    });

    // Fermeture : la page affiche « Enregistrer et quitter / Quitter sans enregistrer / Annuler »
    win.on('close', (e) => {
      if (quitting) return;
      e.preventDefault();
      win.webContents.send('ark:request-quit');
      clearTimeout(quitTimer);
      quitTimer = setTimeout(() => {
        // page bloquée ou pas encore chargée : on ne retient pas l'utilisateur
        quitting = true;
        app.quit();
      }, 4000);
    });
    ipcMain.on('ark:quit-ack', () => clearTimeout(quitTimer));
    ipcMain.on('ark:quit', () => {
      quitting = true;
      app.quit();
    });
    ipcMain.on('ark:open-external', (_e, url) => {
      if (isHttps(url)) shell.openExternal(url);
    });

    // Projets : fenêtres de Windows pour le dossier des projets, ouvrir un fichier, enregistrer sous
    const P = server.projects;
    P.setTrash((p) => shell.trashItem(p));
    // fenêtres de Windows dans la langue de l'interface
    let uiLang = 'fr';
    ipcMain.on('ark:language', (_e, l) => (uiLang = l === 'en' ? 'en' : 'fr'));
    const L = (fr, en) => (uiLang === 'en' ? en : fr);
    const filters = () => [{ name: L('Projet ARKLIGHT', 'ARKLIGHT project'), extensions: ['json'] }];
    const fileName = (n) => String(n || '').replace(/[<>:"/\\|?*\x00-\x1f]/g, '').trim() || L('Nouveau projet', 'New project');
    ipcMain.handle('ark:projects-folder', async () => {
      const r = await dialog.showOpenDialog(win, { title: L('Dossier des projets ARKLIGHT', 'ARKLIGHT projects folder'), defaultPath: P.root(), properties: ['openDirectory', 'createDirectory'] });
      if (r.canceled || !r.filePaths[0]) return null;
      try {
        return { root: P.setRoot(r.filePaths[0]) };
      } catch (e) {
        return { error: e.message };
      }
    });
    ipcMain.handle('ark:project-open', async () => {
      const r = await dialog.showOpenDialog(win, { title: L('Ouvrir un projet ARKLIGHT', 'Open an ARKLIGHT project'), defaultPath: P.root(), properties: ['openFile'], filters: filters() });
      if (r.canceled || !r.filePaths[0]) return null;
      try {
        return P.open(r.filePaths[0]);
      } catch (e) {
        return { error: e.message };
      }
    });
    ipcMain.handle('ark:project-save-as', async (_e, thumbnail) => {
      const current = P.currentPath();
      const r = await dialog.showSaveDialog(win, {
        title: L('Enregistrer le projet sous', 'Save project as'),
        defaultPath: current || join(P.root(), `${fileName(P.suggestedName())}.json`),
        filters: filters(),
      });
      if (r.canceled || !r.filePath) return null;
      const path = /\.json$/i.test(r.filePath) ? r.filePath : `${r.filePath}.json`;
      try {
        return P.saveTo(path, typeof thumbnail === 'string' ? thumbnail : null);
      } catch (e) {
        return { error: e.message };
      }
    });
    ipcMain.handle('ark:project-reveal', (_e, path) => {
      if (typeof path === 'string' && P.isKnown(path)) shell.showItemInFolder(path);
    });
    ipcMain.handle('ark:projects-folder-show', () => shell.openPath(P.root()));

    // test automatique : sans l'écran des projets au démarrage
    await win.loadURL(`${origin}/${process.env.ARKLIGHT_SMOKE ? `?smoke=1${process.env.ARKLIGHT_LANG ? `&lang=${encodeURIComponent(process.env.ARKLIGHT_LANG)}` : ''}` : ''}`);
    trace('page chargée');

    // Test automatique (développement) : capture de la fenêtre puis fermeture. ARKLIGHT_SMOKE_JS :
    // script lancé dans la page avant la capture (ouvrir un menu, une fenêtre…).
    if (process.env.ARKLIGHT_SMOKE) {
      setTimeout(async () => {
        if (process.env.ARKLIGHT_SMOKE_JS) {
          try {
            trace(`script : ${await win.webContents.executeJavaScript(process.env.ARKLIGHT_SMOKE_JS)}`);
          } catch (e) {
            trace(`script en erreur : ${e.message}`);
          }
          await new Promise((r) => setTimeout(r, 800));
        }
        const img = await win.webContents.capturePage();
        writeFileSync(process.env.ARKLIGHT_SMOKE, img.toPNG());
        quitting = true;
        app.quit();
      }, 6000);
    }
  });

  app.on('window-all-closed', () => app.quit());
  app.on('will-quit', () => server?.shutdown?.());
}
