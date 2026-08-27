import { defineConfig } from 'vite';

export default defineConfig({
    resolve: {
        // 0.7.15+ ships an ESM build that imports ./libsodium.mjs from the
        // wrappers package; that file lives in the sibling `libsodium` package.
        // Force the CJS build so the pinned 0.7.13 layout always resolves.
        alias: {
            'libsodium-wrappers': 'libsodium-wrappers/dist/modules/libsodium-wrappers.js',
            'libsodium': 'libsodium/dist/modules/libsodium.js',
        },
    },
    build: {
        manifest: false,
        rollupOptions: {
            // No html entry in v2: the Flutter index.html loads js/dist/index.js.
            input: { index: 'src/main.ts' },
            output: {
                entryFileNames: `[name].js`,
                chunkFileNames: `[name].js`,
                assetFileNames: `[name].[ext]`,
                // Flutter index.html hardcodes js/dist/index.js + js/dist/vendor.js.
                // Restore the Vite <=2.8 vendor split explicitly; the default is
                // gone from 2.9 and splitVendorChunkPlugin was removed in Vite 7.
                manualChunks(id) {
                    if (id.includes('node_modules')) return 'vendor';
                },
            }
        }
    },
})
