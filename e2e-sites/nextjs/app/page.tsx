// Plumb e2e — Next.js fixture. App Router, statically exported.
//
// Same intentional violations as the rest of the matrix: see
// ../README.md.

export default function Page() {
  return (
    <main className="p-6">
      <section className="bg-white text-[#0b0b0b] p-4 mb-6">
        <h1 className="text-2xl font-semibold mb-2">Plumb e2e — nextjs</h1>
        <p className="text-base">
          A minimal Next.js 14 App Router fixture, exported as static
          HTML. The card you are reading is the control element.
        </p>
      </section>

      <section className="bg-white text-[#0b0b0b] p-[13px] mb-6">
        <h2 className="text-xl font-semibold mb-2">Off-grid hero</h2>
        <p className="text-base">
          This region uses an intentionally off-grid arbitrary padding.
        </p>
      </section>

      <p className="bg-white border-[#0b0b0b] text-[#2e7d2e] text-base p-4">
        Off-palette alert text.
      </p>
    </main>
  );
}
