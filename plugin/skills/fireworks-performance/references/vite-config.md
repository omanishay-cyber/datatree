# Vite Configuration — Deep Reference

> Part of the `fireworks-performance` skill. See `../SKILL.md` for the master guide.

---

## Manual Chunks: Vendor Splitting Strategy

Split `node_modules` into logical chunks so that unchanged vendor code stays cached.

### Strategy

```ts
// vite.config.ts
import { defineConfig } from 'vite';

export default defineConfig({
  build: {
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes('node_modules')) {
            // Framework core — changes rarely
            if (id.includes('react') || id.includes('react-dom') || id.includes('react-router')) {
              return 'vendor-react';
            }
            // UI libraries — changes occasionally
            if (id.includes('framer-motion') || id.includes('@radix-ui')) {
              return 'vendor-ui';
            }
            // State management — changes rarely
            if (id.includes('zustand') || id.includes('immer')) {
              return 'vendor-state';
            }
            // Utilities — changes rarely
            if (id.includes('date-fns') || id.includes('clsx') || id.includes('tailwind-merge')) {
              return 'vendor-utils';
            }
            // Everything else from node_modules
            return 'vendor-misc';
          }
        },
      },
    },
  },
});
```

### Alternative: Explicit Object Syntax

```ts
manualChunks: {
  'vendor-react': ['react', 'react-dom', 'react-router-dom'],
  'vendor-ui': ['framer-motion', '@radix-ui/react-dialog', '@radix-ui/react-popover'],
  'vendor-state': ['zustand', 'immer'],
}
```

### When to Use Which

- **Function syntax**: Catches all sub-paths (e.g., `react-dom/client`). More robust.
- **Object syntax**: Simpler, explicit. Use when you know exact package names.

---

## Build Optimization

### Production Build Settings

```ts
export default defineConfig({
  build: {
    // Use esbuild for minification — faster than terser, nearly identical output
    minify: 'esbuild',

    // Split CSS per async chunk — only load CSS for the current route
    cssCodeSplit: true,

    // Show gzipped sizes in build output for accurate size tracking
    reportCompressedSize: true,

    // Warn when a chunk exceeds this size (in KB)
    chunkSizeWarningLimit: 500,

    // Target Electron's Chromium — enables modern syntax, smaller output
    // Check your Electron version's Chrome version at electronjs.org/releases
    target: 'chrome114',

    // Disable source maps in production for smaller output
    // Enable only if you need to debug production issues
    sourcemap: false,

    // Output directory
    outDir: 'dist',

    // Empty the output directory before each build
    emptyOutDir: true,

    // Inline assets smaller than this (in bytes) as base64
    assetsInlineLimit: 4096, // 4KB — good default

    // Rollup options for advanced control
    rollupOptions: {
      output: {
        // Consistent chunk naming for cache busting
        chunkFileNames: 'assets/[name]-[hash].js',
        entryFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash].[ext]',
      },
    },
  },
});
```

### CSS Optimization

```ts
export default defineConfig({
  css: {
    // Enable CSS modules
    modules: {
      localsConvention: 'camelCase',
    },
    // PostCSS configuration
    postcss: './postcss.config.js',
    // Preprocessor options
    preprocessorOptions: {
      // If using SCSS
      scss: {
        additionalData: `@use "@/styles/variables" as *;`,
      },
    },
  },
});
```

---

## Dev Server Configuration

### HMR (Hot Module Replacement)

```ts
export default defineConfig({
  server: {
    // HMR configuration
    hmr: {
      overlay: true,  // Show compilation errors as overlay in browser
    },

    // Pre-transform files the dev server will need
    warmup: {
      // Pre-transform entry points and frequently-used modules
      clientFiles: [
        './src/main.tsx',
        './src/App.tsx',
        './src/components/Layout.tsx',
        './src/stores/*.ts',
      ],
    },

    // Watch options for file system events
    watch: {
      // Use polling in environments where native file watching doesn't work
      // usePolling: true,
      // interval: 1000,
    },
  },
});
```

### Dependency Pre-Bundling

```ts
export default defineConfig({
  optimizeDeps: {
    // Force include deps that Vite might miss during auto-discovery
    include: [
      'react',
      'react-dom',
      'react-router-dom',
      'zustand',
      'framer-motion',
    ],

    // Exclude deps that should NOT be pre-bundled (e.g., if they're ESM-only)
    exclude: ['sql.js'],

    // Force re-optimization when these files change
    entries: ['./src/main.tsx'],
  },
});
```

---

## Multi-Process Electron Builds

Electron apps have THREE processes. Each needs its own build config.

### Project Structure

```
electron-app/
  vite.config.ts          # Renderer (React app)
  vite.main.config.ts     # Main process (Node.js)
  vite.preload.config.ts  # Preload script (bridge)
```

### Renderer Config (React App)

```ts
// vite.config.ts
export default defineConfig({
  root: './src/renderer',
  build: {
    outDir: '../../dist/renderer',
    target: 'chrome114',
    minify: 'esbuild',
    cssCodeSplit: true,
  },
  plugins: [react()],
  resolve: {
    alias: {
      '@': resolve(__dirname, 'src/renderer'),
    },
  },
});
```

### Main Process Config

```ts
// vite.main.config.ts
export default defineConfig({
  build: {
    outDir: 'dist/main',
    target: 'node20',          // Node.js target for Electron main
    lib: {
      entry: 'src/main/index.ts',
      formats: ['cjs'],        // Main process uses CommonJS
    },
    rollupOptions: {
      external: ['electron', 'sql.js', 'better-sqlite3'], // Don't bundle native modules
    },
    minify: false,             // No need to minify server-side code
    sourcemap: true,           // Useful for debugging main process
  },
});
```

### Preload Config

```ts
// vite.preload.config.ts
export default defineConfig({
  build: {
    outDir: 'dist/preload',
    target: 'node20',
    lib: {
      entry: 'src/preload/index.ts',
      formats: ['cjs'],
    },
    rollupOptions: {
      external: ['electron'],
    },
    minify: false,
    sourcemap: true,
  },
});
```

---

## Environment Variables and Dead Code Elimination

### Compile-Time Replacement

```ts
export default defineConfig({
  define: {
    // These become compile-time constants — Vite replaces them literally
    __DEV__: JSON.stringify(process.env.NODE_ENV !== 'production'),
    __APP_VERSION__: JSON.stringify(process.env.npm_package_version),
    __BUILD_TIME__: JSON.stringify(new Date().toISOString()),
  },
});
```

### Usage in Code

```ts
// This code is COMPLETELY removed in production builds
if (__DEV__) {
  console.log('Debug: component mounted with props', props);
  window.__DEBUG_STORE__ = store;
}

// Version display
function About() {
  return <p>Version: {__APP_VERSION__}</p>;
}
```

### How It Works

Vite replaces `__DEV__` with `false` in production. Then the minifier sees:

```ts
if (false) {
  console.log('Debug: ...');
}
```

And removes the entire block as dead code. Zero runtime cost.

---

## Asset Handling

### Inline Small Assets

```ts
export default defineConfig({
  build: {
    // Files smaller than this are inlined as base64 data URLs
    // Saves HTTP requests for tiny images/icons
    assetsInlineLimit: 4096, // 4KB
  },
});
```

### Public Directory

```ts
export default defineConfig({
  // Files in publicDir are copied as-is to the output directory
  // Use for files that must keep their exact filename (favicon.ico, robots.txt)
  publicDir: 'public',
});
```

### Asset Import

```ts
// Import as URL — Vite adds hash for cache busting
import logoUrl from './assets/logo.png';
// Result: /assets/logo-a1b2c3d4.png

// Import as raw string
import shaderCode from './shaders/fragment.glsl?raw';

// Import as worker
import Worker from './worker.ts?worker';
```
