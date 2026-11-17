import type { NextConfig } from "next"

// The UI is a static export bundled into the Tauri app (frontendDist ../out): no server, no remote origin.
const nextConfig: NextConfig = {
  output: "export",
  images: { unoptimized: true },
  trailingSlash: true,
}

export default nextConfig
