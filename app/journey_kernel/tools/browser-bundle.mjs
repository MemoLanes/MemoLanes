import { mkdtemp, writeFile, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { createServer } from "node:http";
import { once } from "node:events";
import path from "node:path";
import { fileURLToPath } from "node:url";
import webpack from "webpack";

export const kernelRoot = fileURLToPath(new URL("..", import.meta.url));

/** Serve a temporary browser tool bundle; nothing is written into app assets. */
export async function serveBrowserBundle(entry, html) {
  const output = await mkdtemp(path.join(tmpdir(), "journey-browser-"));
  let server;
  async function close() {
    if (server?.listening) {
      const closed = once(server, "close");
      server.close();
      server.closeAllConnections();
      await closed;
    }
    await rm(output, { recursive: true, force: true });
  }
  try {
    const compiler = webpack({
      context: kernelRoot,
      mode: "development",
      devtool: false,
      entry,
      output: { path: output, filename: "bundle.js" },
      resolve: { extensions: [".ts", ".js"] },
      module: {
        rules: [
          {
            test: /\.ts$/,
            use: { loader: "ts-loader", options: { transpileOnly: true } },
            exclude: /node_modules/,
          },
          { test: /\.glsl$/, type: "asset/source" },
          { test: /\.css$/, use: ["style-loader", "css-loader"] },
          { test: /\.wasm$/, type: "asset/inline" },
        ],
      },
    });
    try {
      await new Promise((resolve, reject) =>
        compiler.run((error, stats) => {
          if (error || stats.hasErrors())
            reject(
              error ?? new Error(stats.toString({ all: false, errors: true })),
            );
          else resolve();
        }),
      );
    } finally {
      await new Promise((resolve, reject) =>
        compiler.close((error) => (error ? reject(error) : resolve())),
      );
    }
    await writeFile(path.join(output, "index.html"), html);
    for (const name of ["maplibre-gl-worker.mjs", "maplibre-gl-shared.mjs"]) {
      await writeFile(
        path.join(output, name),
        await readFile(
          path.join(kernelRoot, "node_modules/maplibre-gl/dist", name),
        ),
      );
    }
    const assets = new Set(await readdir(output));
    server = createServer(async (request, response) => {
      const name =
        new URL(request.url, "http://localhost").pathname.slice(1) ||
        "index.html";
      if (name === "favicon.ico") {
        response.writeHead(204).end();
        return;
      }
      if (!assets.has(name)) {
        response.writeHead(404).end();
        return;
      }
      try {
        const bytes = await readFile(path.join(output, name));
        response.setHeader(
          "Content-Type",
          name.endsWith(".html") ? "text/html" : "text/javascript",
        );
        response.end(bytes);
      } catch {
        response.writeHead(500).end();
      }
    });
    server.listen(0, "127.0.0.1");
    await once(server, "listening");
    return { url: `http://127.0.0.1:${server.address().port}`, close };
  } catch (error) {
    await close();
    throw error;
  }
}
