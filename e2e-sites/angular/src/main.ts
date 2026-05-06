import { bootstrapApplication } from "@angular/platform-browser";
import { AppComponent } from "./app/app.component";

bootstrapApplication(AppComponent).catch((err: unknown) => {
  // eslint-disable-next-line no-console
  console.error(err);
});
