import { defineConfig } from 'vite';

export default defineConfig({
    resolve: {
        // 0.7.15+ ships an ESM build that imports ./libsodium.mjs from the
        // wrappers package; that file lives in the sibling `libsodium` package.
        // Force the CJS build so the frozen 0.7.13 layout always resolves.
        alias: {
            'libsodium-wrappers': 'libsodium-wrappers/dist/modules/libsodium-wrappers.js',
            'libsodium': 'libsodium/dist/modules/libsodium.js',
        },
    },
    build: {
        manifest: false,
        rollupOptions: {
            output: {
                entryFileNames: `[name].js`,
                chunkFileNames: `[name].js`,
                assetFileNames: `[name].[ext]`,
                // Flutter index.html hardcodes js/dist/index.js + js/dist/vendor.js.
                // Vite 2.8 default-split vendor; that default is gone from 2.9, and
                // splitVendorChunkPlugin was removed in Vite 7. Restore it here.
                manualChunks(id) {
                    if (id.includes('node_modules')) return 'vendor';
                },
            }
        }
    },
})