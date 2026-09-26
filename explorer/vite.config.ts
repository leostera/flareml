import { defineConfig } from "vite";
const origin = process.env.FML_EXPLORER_ORIGIN;
if (origin) {
  const url = new URL(origin);
  if (
    url.protocol !== "http:" ||
    url.hostname !== "127.0.0.1" ||
    !url.port ||
    url.username ||
    url.password ||
    url.pathname !== "/" ||
    url.search ||
    url.hash
  ) {
    throw new Error(
      "FML_EXPLORER_ORIGIN must be http://127.0.0.1:<native-server-port> (without the token).",
    );
  }
}
export default defineConfig({
  server: {
    proxy: origin
      ? { "/api/": { target: origin, changeOrigin: true } }
      : undefined,
  },
  plugins: [
    {
      name: "explain-missing-native-server",
      configureServer(server) {
        if (!origin)
          server.middlewares.use((request, response, next) => {
            if (!request.url?.startsWith("/api/")) return next();
            response.statusCode = 503;
            response.setHeader("Content-Type", "text/plain; charset=utf-8");
            response.end(
              "No native replay server configured. Start fml replay <model> <trace> --ui --no-open, then run FML_EXPLORER_ORIGIN=http://127.0.0.1:<printed-port> bun run dev. Copy the printed # token into the development URL.",
            );
          });
      },
    },
  ],
  build: {
    sourcemap: false,
    rollupOptions: {
      output: {
        entryFileNames: "assets/app.js",
        inlineDynamicImports: true,
        assetFileNames: (asset) =>
          asset.names?.some((n) => n.endsWith(".css"))
            ? "assets/app.css"
            : "assets/[name][extname]",
      },
    },
  },
});
