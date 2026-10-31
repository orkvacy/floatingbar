import { defineConfig, type Plugin, type ViteDevServer } from "vite";
import react from "@vitejs/plugin-react";
import type { IncomingMessage, ServerResponse } from "node:http";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { getDiscordBridge } = require("./src/server/discord-bridge.cjs");

function discordBridgePlugin(): Plugin {
  return {
    name: "discord-bridge-plugin",
    configureServer(server: ViteDevServer) {
      server.middlewares.use(async (req: IncomingMessage, res: ServerResponse, next: () => void) => {
        const url = req.url ? req.url.split("?")[0] : "";
        if (!url.startsWith("/api/discord/")) return next();
        // The desktop uses Rust IPC. Start the Node bridge only for browser clients,
        // otherwise tauri dev opens two Discord sessions and authorization flows.
        const bridge = getDiscordBridge();

        if (url === "/api/discord/events") {
          res.writeHead(200, {
            "Content-Type": "text/event-stream",
            "Cache-Control": "no-cache",
            Connection: "keep-alive",
            "Access-Control-Allow-Origin": "*",
          });
          res.write("\n");
          bridge.addSubscriber(res);

          req.on("close", () => {
            bridge.removeSubscriber(res);
          });
          return;
        }

        if (url === "/api/discord/state" && req.method === "GET") {
          res.writeHead(200, {
            "Content-Type": "application/json",
            "Access-Control-Allow-Origin": "*",
          });
          res.end(JSON.stringify(bridge.state));
          return;
        }

        if (url.startsWith("/api/discord/") && req.method === "POST") {
          let body = "";
          req.on("data", (chunk: Buffer) => (body += chunk));
          req.on("end", () => {
            try {
              const data = body ? JSON.parse(body) : {};
              const action = url.replace("/api/discord/", "");

              if (action === "mute") {
                bridge.setMute(data.mute);
              } else if (action === "deaf") {
                bridge.setDeaf(data.deaf);
              } else if (action === "user-volume") {
                bridge.setUserVolume(data.userId, data.volume);
              } else if (action === "user-mute") {
                bridge.toggleUserMute(data.userId, data.mute);
              } else if (action === "disconnect") {
                bridge.disconnect();
              }

              res.writeHead(200, {
                "Content-Type": "application/json",
                "Access-Control-Allow-Origin": "*",
              });
              res.end(JSON.stringify({ ok: true }));
            } catch (err: any) {
              res.writeHead(400, { "Content-Type": "application/json" });
              res.end(JSON.stringify({ error: err?.message || String(err) }));
            }
          });
          return;
        }

        next();
      });
    },
  };
}


// Tauri expects a fixed port; it fails if the port is taken.
export default defineConfig({
  plugins: [react(), discordBridgePlugin()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
});

