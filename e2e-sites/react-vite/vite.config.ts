import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    // Disable minify-mangling of class names so the JIT-emitted Tailwind
    // utilities round-trip through Plumb's lint exactly as authored. The
    // matrix is about computed-style invariants, not bundle size.
    minify: false,
  },
});
