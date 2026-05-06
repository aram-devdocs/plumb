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
  return (
    <html lang="en">
      <body className="bg-white text-[#0b0b0b]">{children}</body>
    </html>
  );
}
