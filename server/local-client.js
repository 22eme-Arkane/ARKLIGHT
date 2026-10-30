// Qui a les droits « de cet ordinateur » (assistant IA : clés d'API, AIODOS, adresses des
// fournisseurs) : l'interface d'ARKLIGHT ouverte sur ce PC, et elle seule.
//
// L'adresse ne suffit pas : une page web ouverte dans un navigateur de ce PC se connecte, elle
// aussi, depuis 127.0.0.1 (ws://localhost:8686/ws). Elle pourrait alors se servir d'AIODOS à
// travers ARKLIGHT, ou changer l'adresse d'un fournisseur pour y recevoir sa clé d'API. Il faut
// donc en plus que la demande vienne de l'interface servie par ARKLIGHT lui-même (son origine,
// http://localhost:<port>, que charge aussi l'application de bureau), ou d'un programme qui n'est
// pas un navigateur (pas d'en-tête Origin : un navigateur l'envoie toujours pour un WebSocket et
// pour un POST).

/** Adresse de cet ordinateur (127.0.0.1, ::1, ::ffff:127.0.0.1). */
export const isLoopback = (a) => /^(127\.|::1$|::ffff:127\.)/.test(String(a || ''));

/** Origines de l'interface servie par ARKLIGHT sur ce port (localhost, 127.0.0.1, [::1]). */
export function uiOrigins(port) {
  const p = Number(port);
  const ports = p === 80 ? ['', ':80'] : [`:${p}`];
  return ['localhost', '127.0.0.1', '[::1]'].flatMap((host) => ports.map((s) => `http://${host}${s}`));
}

/**
 * Vrai pour une connexion WebSocket ou une requête HTTP de l'interface ouverte sur cet ordinateur :
 * adresse locale, et origine absente ou égale à celle de l'interface d'ARKLIGHT (`port` : le port
 * où ARKLIGHT écoute vraiment). Toute autre origine est refusée, « null » compris (fichier ouvert
 * dans le navigateur, cadre isolé), comme une adresse qui viendrait d'une autre machine
 * (DNS rebinding : http://site.example:8686 qui pointe vers 127.0.0.1).
 * @param {import('node:http').IncomingMessage} req
 * @param {number} port
 */
export function isLocalClient(req, port) {
  if (!isLoopback(req?.socket?.remoteAddress)) return false;
  const origin = req.headers?.origin;
  if (origin === undefined) return true;
  return uiOrigins(port).includes(String(origin).trim().toLowerCase());
}
