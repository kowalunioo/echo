import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./App";
import { initI18n, uiLanguageForLocale } from "./i18n";
import { useShell } from "./store/shell";
import "./styles.css";

// First paint uses the WebView's language (it follows Windows); the backend's answer from
// `app_info` then confirms or corrects it.
const initial = uiLanguageForLocale(navigator.language);
useShell.setState({ uiLanguage: initial });
initI18n(initial);

const root = document.getElementById("root");
if (!root) throw new Error("#root is missing from index.html");

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
);
