// @ts-nocheck — Vite plugin filter shapes vary across versions
import { defineConfig } from "vite";
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

/**
 * @tailwindcss/vite matches `?…&lang.css` and runs with enforce:"pre".
 * For Svelte virtual CSS modules (`*.svelte?svelte&type=style&lang.css`), Vite can
 * hand Tailwind the full .svelte source (script + markup) before the Svelte plugin
 * extracts CSS — which produces "Invalid declaration: `goto`/`page`/`settings`".
 *
 * Component styles live in app.css; we don't need Tailwind on those virtual modules.
 */
function tailwindcssForSvelte() {
  const plugins = tailwindcss();
  const skipSvelteVirtual = /[?&]svelte(?:&|$|=)/;

  return plugins.map((plugin) => {
    if (!plugin?.name?.startsWith("@tailwindcss/vite:generate")) return plugin;

    const transform = plugin.transform;
    if (!transform || typeof transform !== "object" || !transform.handler) {
      return plugin;
    }

    const prevExclude = transform.filter?.id?.exclude;
    const excludeList = [
      ...(Array.isArray(prevExclude)
        ? prevExclude
        : prevExclude
          ? [prevExclude]
          : []),
      skipSvelteVirtual,
    ];

    const prevHandler = transform.handler;
    return {
      ...plugin,
      transform: {
        ...transform,
        filter: {
          ...transform.filter,
          id: {
            ...(typeof transform.filter?.id === "object"
              ? transform.filter.id
              : {}),
            exclude: excludeList,
          },
        },
        async handler(code, id) {
          // Filter + content guard: never compile raw Svelte/JS as CSS.
          if (
            skipSvelteVirtual.test(id) ||
            code.includes("<script") ||
            /^\s*import\s/.test(code)
          ) {
            return;
          }
          return prevHandler.call(this, code, id);
        },
      },
    };
  });
}

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [...tailwindcssForSvelte(), sveltekit()],

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));
