// ARKLIGHT — application de bureau (Electron) : démarre le serveur du simulateur et l'affiche
// dans sa propre fenêtre. À la fermeture, la page propose d'enregistrer la scène (et de soutenir
// le projet), comme PANDORA.
import { app, BrowserWindow, ipcMain, Menu, shell } from 'electron';
import { appendFileSync, existsSync, renameSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const ROOT = join(here, '..');
const isHttps = (url) => /^https:\/\//i.test(String(url));
/** Journal du test automatique (ARKLIGHT_SMOKE) : la console d'Electron n'est pas visible sous Windows. */
const trace = (msg) => {
  if (process.env.ARKLIGHT_SMOKE) appendFileSync(`${process.env.ARKLIGHT_SMOKE}.log`, `${new Date().toISOString()} ${msg}
`);
};
process.on('uncaughtException', (e) => trace(`erreur : ${e.stack || e}`));
process.on('unhandledRejection', (e) => trace(`promesse rejetée : ${e?.stack || e}`));

if (!app.requestSingleInstanceLock()) {
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
      webPreferences: { preload: join(here, 'preload.cjs'), contextIsolation: true, nodeIntegration: false },
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

    await win.loadURL(`${origin}/`);
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
