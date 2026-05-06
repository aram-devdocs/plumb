// Plumb e2e — React fixture. The component renders the same layout as
// `e2e-sites/html-css/index.html` so the harness asserts identical
// counts across stacks. Two intentional violation sources:
//
//   - `.hero` uses `p-[13px]` (off-grid + off-scale).
//   - `.alert` uses `text-[#2e7d2e]` (off-palette). `border-[#0b0b0b]`
//     pins the four `border-*-color` longhands to a palette token so
//     they don't inherit the off-palette `currentColor`.

export default function App() {
  return (
    <main className="p-6">
      <section className="bg-white text-[#0b0b0b] p-4 mb-6">
        <h1 className="text-2xl font-semibold mb-2">
          Plumb e2e — react-vite
        </h1>
        <p className="text-base">
          A minimal React + Vite + Tailwind fixture. The card you are
          reading is the control element.
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
