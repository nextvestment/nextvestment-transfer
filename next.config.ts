import type { NextConfig } from "next";
import path from "node:path";

const nextConfig: NextConfig = {
  // Required for Tauri - static site generation
  output: 'export',

  turbopack: {
    root: path.resolve(__dirname),
  },
  
  // Required for static export
  images: {
    unoptimized: true
  },
  
  // Disable trailing slash for cleaner URLs
  trailingSlash: false,
};

export default nextConfig;
