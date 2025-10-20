import * as esbuild from 'esbuild'

await esbuild.build({
  entryPoints: [
    "web/src/money.ts",
    "web/index.html",
    "web/money.css",
  ],
  loader: {
    ".html": "copy",
    ".css": "copy",
  },
  bundle: true,
  minify: true,
  sourcemap: true,
  outdir: "assets",
  entryNames: "[name]"
});
