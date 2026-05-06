/** @type {import('next').NextConfig} */
const nextConfig = {
  // Static HTML export — `next build` writes a self-contained `out/`
  // directory the harness's static server can serve as-is.
  output: "export",
  // Disable image optimization since the export target has no Node
  // server. The fixture has no <Image> usages anyway, but Next refuses
  // to build with the default `remotePatterns` config under `output:
  // 'export'` if any image component is used.
  images: { unoptimized: true },
  // Asset prefix — keep relative so the bundle works under any path
  // (including `https://plumb.aramhammoudeh.com/test-sites/nextjs/`).
  trailingSlash: true,
};

module.exports = nextConfig;
