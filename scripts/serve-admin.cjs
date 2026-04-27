#!/usr/bin/env node
/**
 * Serves fitness-sbt root over HTTP so wallet extensions work with file:// limitations.
 * Open http://127.0.0.1:<port>/ (defaults to admin-final.html).
 *
 *   npm run admin:serve
 *   PORT=9000 npm run admin:serve
 */
const http = require('http');
const fs = require('fs');
const path = require('path');

const ROOT = path.join(__dirname, '..');
const PORT = Number(process.env.PORT || 8787);
const HOST = process.env.HOST || '127.0.0.1';

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.htm': 'text/html; charset=utf-8',
  '.js': 'application/javascript',
  '.mjs': 'application/javascript',
  '.json': 'application/json',
  '.css': 'text/css',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.ico': 'image/x-icon',
  '.webp': 'image/webp',
  '.md': 'text/markdown; charset=utf-8',
};

function safeJoin(root, reqPath) {
  const decoded = decodeURIComponent(reqPath.split('?')[0]);
  const rel = decoded.replace(/^\/+/, '');
  const fp = path.normalize(path.join(root, rel));
  if (!fp.startsWith(root)) return null;
  return fp;
}

const server = http.createServer((req, res) => {
  let rel = req.url.split('?')[0];
  if (rel === '/' || rel === '') rel = '/admin-final.html';

  const fp = safeJoin(ROOT, rel);
  if (!fp) {
    res.writeHead(403);
    res.end('Forbidden');
    return;
  }

  fs.stat(fp, (err, st) => {
    if (err || !st.isFile()) {
      res.writeHead(404, { 'Content-Type': 'text/plain' });
      res.end('Not found: ' + rel);
      return;
    }
    fs.readFile(fp, (e, data) => {
      if (e) {
        res.writeHead(500);
        res.end('Read error');
        return;
      }
      const ext = path.extname(fp).toLowerCase();
      res.writeHead(200, {
        'Content-Type': MIME[ext] || 'application/octet-stream',
        'Cache-Control': 'no-store',
      });
      res.end(data);
    });
  });
});

server.listen(PORT, HOST, () => {
  const base = `http://${HOST}:${PORT}`;
  // eslint-disable-next-line no-console
  console.log('');
  console.log('  Sanitas admin (local)');
  console.log('  --------------------');
  console.log(`  ${base}/`);
  console.log(`  ${base}/admin-final.html   ← multisig upgrade (governor)`);
  console.log(`  ${base}/docs/index.html     ← legal static site (if present)`);
  console.log('');
  console.log('  Use a browser on this machine. Connect Phantom / Backpack to devnet.');
  console.log('  Governor Program ID & target app id are in admin-final.html defaults.');
  console.log('');
});
