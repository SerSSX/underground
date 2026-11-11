/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  images: { unoptimized: true },
  // A self-contained server (node server.js) with only the files it needs: a small runtime image.
  output: "standalone",
  // The commit the bundle was built from. The App Router embeds the build id in each page's HTML, so the
  // deploy smoke (deploy/smoke-dev.sh) can prove the served build is the commit it just deployed.
  generateBuildId: async () => process.env.UG_BUILD_SHA || null,
};

export default nextConfig;
