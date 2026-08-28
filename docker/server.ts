// Static file server for the IdleBot wasm client.
// Run with: bun run server.ts  (expects files in ./public)

const PORT = Number(Bun.env.PORT ?? 3012);

const MIME: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".gif": "image/gif",
  ".webp": "image/webp",
  ".svg": "image/svg+xml",
  ".glb": "model/gltf-binary",
  ".gltf": "model/gltf+json",
  ".ttf": "font/ttf",
  ".otf": "font/otf",
  ".woff": "font/woff",
  ".woff2": "font/woff2",
};

const server = Bun.serve({
  port: PORT,
  async fetch(req) {
    const url = new URL(req.url);
    let path = decodeURIComponent(url.pathname);
    if (path === "/") path = "/index.html";

    const file = Bun.file("public" + path);
    if (await file.exists()) {
      const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
      const headers: Record<string, string> = {
        "content-type": MIME[ext] ?? "application/octet-stream",
      };
      // Don't cache the code/glue; assets are content-addressed by the loader.
      if (ext === ".wasm" || ext === ".js") {
        headers["cache-control"] = "no-cache";
      } else {
        headers["cache-control"] = "public, max-age=604800";
      }
      return new Response(file, { headers });
    }
    return new Response("Not found", { status: 404 });
  },
});

console.log(`IdleBot web client serving on http://localhost:${server.port}`);
