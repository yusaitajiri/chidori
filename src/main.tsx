import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import "./i18n";
// The brand's fonts, bundled: nothing is fetched from the network.
import "@fontsource/zen-kaku-gothic-new/400.css";
import "@fontsource/zen-kaku-gothic-new/500.css";
import "@fontsource/ibm-plex-mono/400.css";
import "./index.css";

// The webview's default context menu (Reload, Inspect) makes no sense in this app.
if (!import.meta.env.DEV) {
  window.addEventListener("contextmenu", (e) => {
    const target = e.target as HTMLElement;
    if (!target.closest("input, textarea, .selectable")) e.preventDefault();
  });
}

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
