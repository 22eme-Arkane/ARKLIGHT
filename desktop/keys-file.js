// Clés de l'assistant IA (clé d'ARKLIGHT pour AIODOS, clés d'API d'Anthropic, d'OpenAI…,
// chiffrées par safeStorage) : rangées dans le profil de l'application (app.getPath('userData'),
// %APPDATA%\arklight), jamais dans Documents, souvent synchronisé par OneDrive (voir
// docs/integrer-aiodos.md § 5 d'AIODOS). Les premières versions les mettaient dans
// Documents\ARKLIGHT : le fichier en est déplacé au premier lancement.
import { copyFileSync, existsSync, mkdirSync, renameSync, rmSync, statSync } from 'node:fs';
import { dirname, resolve } from 'node:path';

const samePath = (a, b) => (process.platform === 'win32' ? resolve(a).toLowerCase() === resolve(b).toLowerCase() : resolve(a) === resolve(b));

/** Déplace un fichier, même d'un disque à l'autre, sans le perdre en route. */
function moveFile(from, to) {
  try {
    renameSync(from, to);
    return;
  } catch (e) {
    if (e.code !== 'EXDEV') throw e;
  }
  // autre disque : copie complète d'abord (fichier temporaire, puis renommage), effacement ensuite
  const tmp = `${to}.tmp`;
  try {
    copyFileSync(from, tmp);
    renameSync(tmp, to);
  } catch (e) {
    rmSync(tmp, { force: true });
    throw e;
  }
  rmSync(from);
}

/**
 * Range le fichier des clés à `file` (le nouvel emplacement) en y déplaçant `legacy` (l'ancien),
 * s'il existe. S'il existe aux deux endroits, le plus récent est gardé à `file` et l'autre est
 * remplacé ou effacé ; rien d'autre n'est touché. Rend le chemin à utiliser pour cette fois :
 * `file`, ou `legacy` si son déplacement a échoué (fichier ouvert ailleurs…) alors qu'il est le
 * seul ou le plus récent : rien n'est perdu, on réessaie au prochain lancement.
 * @param {string} file
 * @param {string} [legacy]
 * @returns {string}
 */
export function placeKeyFile(file, legacy) {
  if (!legacy || samePath(file, legacy) || !existsSync(legacy)) return file;
  let keepNew = false;
  try {
    keepNew = existsSync(file) && statSync(file).mtimeMs >= statSync(legacy).mtimeMs;
    if (keepNew) rmSync(legacy);
    else {
      mkdirSync(dirname(file), { recursive: true });
      moveFile(legacy, file);
    }
    return file;
  } catch {
    return keepNew ? file : legacy;
  }
}
