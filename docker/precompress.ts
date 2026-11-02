// Pre-compresses the static bundle (brotli first, gzip fallback) so the
// browser downloads the wasm/assets compressed. Bun.serve serves the
// `.br`/`.gz` siblings when the request's Accept-Encoding allows it, which
// shrinks the ~1-3 MB wasm download to roughly a third without changing
// how the game is built or loaded.
import * as fs from "fs";
import * as path from "path";
import { gzipSync, brotliCompressSync, constants as zlibConstants } from "node:zlib";

const COMPRESSIBLE = new Set([
  ".wasm", ".js", ".mjs", ".html", ".css", ".json",
  ".svg", ".txt", ".ttf", ".otf", ".woff", ".woff2",
]);

function compress(data: Buffer, kind: "br" | "gz"): Buffer {
  return kind === "br"
    ? brotliCompressSync(data, { params: { [zlibConstants.BROTLI_PARAM_QUALITY]: 5 } })
    : gzipSync(data, { level: 9 });
}

let count = 0;
function precompress(dir: string) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      precompress(full);
    } else if (COMPRESSIBLE.has(path.extname(entry.name).toLowerCase())) {
      const data = fs.readFileSync(full);
      for (const kind of ["br", "gz"] as const) {
        const out = compress(data, kind);
        if (out.length < data.length) {
          fs.writeFileSync(full + "." + kind, out);
          count++;
        }
      }
    }
  }
}

precompress(process.argv[2] ?? "public");
console.log(`precompressed ${count} files`);
