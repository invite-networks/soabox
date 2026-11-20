import "@fontsource-variable/inter";
import "@fontsource-variable/jetbrains-mono";
import "./styles/app.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App, createApp } from "./app";
import { applyTheme, readThemeChoice } from "./lib/theme";

applyTheme(readThemeChoice());

const container = document.getElementById("root");
if (!container) {
  throw new Error("The #root element is missing from index.html.");
}

const { router, queryClient } = createApp();

createRoot(container).render(
  <StrictMode>
    <App router={router} queryClient={queryClient} />
  </StrictMode>,
);
