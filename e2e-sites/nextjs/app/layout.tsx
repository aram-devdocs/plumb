import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Plumb e2e — nextjs fixture",
};

export default function RootLayout({
  children,
}: {
  children: React.ReactNode;
}) {
  // `data-plumb-ready="true"` is server-rendered directly onto the
  // `<html>` element so the harness's `wait_for` selector resolves
  // against the very first paint. No `'use client'` component is
  // needed; the resulting `out/index.html` is a deterministic static
  // artifact, byte-stable across Linux/macOS/Windows builds.
  return (
    <html lang="en" data-plumb-ready="true">
      <body className="bg-white text-[#0b0b0b]">{children}</body>
    </html>
  );
}
