import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { resolve } from "path";

export default defineConfig({
    plugins: [react()],
    server: {
        port: 5173,
        strictPort: true
    },
    build: {
        rollupOptions: {
            input: {
                main: resolve(__dirname, "index.html"),
                picker: resolve(__dirname, "picker.html"),
            },
        },
    },
});


