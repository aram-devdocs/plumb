import { Component } from "@angular/core";

@Component({
  selector: "app-root",
  standalone: true,
  template: `
    <main class="p-6">
      <section class="bg-white text-[#0b0b0b] p-4 mb-6">
        <h1 class="text-2xl font-semibold mb-2">Plumb e2e — angular</h1>
        <p class="text-base">
          A minimal Angular 17 standalone-component fixture. The card
          you are reading is the control element.
        </p>
      </section>

      <section class="bg-white text-[#0b0b0b] p-[13px] mb-6">
        <h2 class="text-xl font-semibold mb-2">Off-grid hero</h2>
        <p class="text-base">
          This region uses an intentionally off-grid arbitrary padding.
        </p>
      </section>

      <p class="bg-white border-[#0b0b0b] text-[#2e7d2e] text-base p-4">
        Off-palette alert text.
      </p>
    </main>
  `,
})
export class AppComponent {}
