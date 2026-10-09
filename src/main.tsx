import { getCurrentWindow } from "@tauri-apps/api/window";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { initI18n, uiLanguageForLocale } from "./i18n";
import { OverlayApp } from "./overlay/OverlayApp";
import { useSettings } from "./store/settings";
import "./styles.css";

// Load the settings before the first paint so the window opens in the saved UI Language. If
// they cannot be loaded, the WebView's language (it follows Windows) is the best guess.
await useSettings.getState().load();
initI18n(useSettings.getState().settings?.uiLanguage ?? uiLanguageForLocale(navigator.language));

const root = document.getElementById("root");
if (!root) throw new Error("#root is missing from the page");

// index.html serves the main window, overlay.html the Overlay window (overlay.md); both run this.
const overlay = getCurrentWindow().label === "overlay";

createRoot(root).render(<StrictMode>{overlay ? <OverlayApp /> : <App />}</StrictMode>);
