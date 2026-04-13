import { defineConfig, type Plugin } from "vite";
import { resolve } from "path";
import { writeFileSync } from "fs";
import react from "@vitejs/plugin-react";

// Generate a standalone index.html that loads the library bundle
function standaloneHtml(): Plugin {
  return {
    name: "standalone-html",
    closeBundle() {
      const html = `<!DOCTYPE html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Knowledgebase</title>
    <link rel="preconnect" href="https://fonts.googleapis.com" />
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
    <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700&display=swap" rel="stylesheet" />
    <link rel="stylesheet" href="./kb.css" />
    <style>
      *, *::before, *::after { box-sizing: border-box; }
      body { margin: 0; font-family: Inter, system-ui, sans-serif; background: #f0f0f3; color: #111827; -webkit-font-smoothing: antialiased; }
    </style>
  </head>
  <body>
    <div id="ms-mount" style="height: 100vh; padding: 1.5rem;"></div>
    <script type="module">
      import { mount } from "./kb.js";
      mount(document.getElementById("ms-mount"));
    </script>
  </body>
</html>`;
      writeFileSync(resolve(__dirname, "dist/index.html"), html);
    },
  };
}

export default defineConfig({
  plugins: [react(), standaloneHtml()],
  define: {
    "process.env.NODE_ENV": JSON.stringify("production"),
  },
  build: {
    lib: {
      entry: resolve(__dirname, "src/main.tsx"),
      formats: ["es"],
      fileName: "kb",
    },
    cssFileName: "kb",
  },
  server: {
    port: 5174,
    proxy: {
      "/api": {
        target: "http://localhost:4001",
        changeOrigin: true,
      },
    },
  },
});
