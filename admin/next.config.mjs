// Security headers on every response (TASK-305 F5). Set by the app, not only by the proxy, so the build CI
// tests is the one that ships. No script-src: Next's inline bootstrap scripts would need nonces (out of scope).
const securityHeaders = [
  { key: "Strict-Transport-Security", value: "max-age=15552000" },
  { key: "X-Frame-Options", value: "DENY" },
  { key: "Content-Security-Policy", value: "frame-ancestors 'none'; object-src 'none'; base-uri 'self'" },
  { key: "X-Content-Type-Options", value: "nosniff" },
  { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
];

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  // No "X-Powered-By: Next.js" banner.
  poweredByHeader: false,
  async headers() {
    return [{ source: "/:path*", headers: securityHeaders }];
  },
  // A self-contained server (node server.js) with only the files it needs: a small runtime image.
  output: "standalone",
  // The commit the bundle was built from. The App Router embeds the build id in each page's HTML, so the
  // deploy smoke (deploy/smoke-dev.sh) can prove the served build is the commit it just deployed.
  generateBuildId: async () => process.env.UG_BUILD_SHA || null,
};

export default nextConfig;
