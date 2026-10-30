// Textes pas encore traduits, pour une liste de fichiers : node tools/i18n/todo.mjs web/js/actions.js web/js/ui/inspector.js
// Sans argument : tous les fichiers.
import { collect } from './extract.mjs';
import { EN } from '../../shared/i18n/en.js';
import { lookup } from '../../shared/i18n.js';

const files = process.argv.slice(2);
const todo = collect().filter((x) => !(x.text in EN) && lookup(x.text, 'en') == null && (!files.length || x.files.some((f) => files.includes(f))));
console.log(`${todo.length} textes à traduire`);
for (const x of todo) console.log(JSON.stringify(x.text));
