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

    const enc = (req.headers.get("accept-encoding") ?? "").toLowerCase();
    // .wasm gets gzip only (universal wasm decoding); text assets prefer brotli.
    const precompressed = path.endsWith(".wasm")
      ? (enc.includes("gzip") ? path + ".gz" : null)
      : enc.includes("br")
        ? path + ".br"
        : enc.includes("gzip")
          ? path + ".gz"
          : null;

    let file = precompressed && (await Bun.file("public" + precompressed).exists())
      ? Bun.file("public" + precompressed)
      : Bun.file("public" + path);
    if (await file.exists()) {
      const ext = path.slice(path.lastIndexOf(".")).toLowerCase();
      const headers: Record<string, string> = {
        "content-type": MIME[ext] ?? "application/octet-stream",
      };
      // Serve a precompressed sibling (brotli first, gzip fallback).
      if (file.name.endsWith(".br")) {
        headers["content-encoding"] = "br";
      } else if (file.name.endsWith(".gz")) {
        headers["content-encoding"] = "gzip";
      }
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
