import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { updateTheme } from "./utils.ts";

import "./index.css";
import App from "./App.tsx";

window
  .matchMedia("(prefers-color-scheme: dark)")
  .addEventListener("change", () => updateTheme());
updateTheme();

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
