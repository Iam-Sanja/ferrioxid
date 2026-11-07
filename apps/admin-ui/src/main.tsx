import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { BrowserRouter } from "react-router-dom";
import { App } from "./App";
import { AuthProvider } from "./auth/AuthProvider";
import { I18nProvider } from "./i18n/provider";
import "./styles/app.css";

createRoot(document.getElementById("root")!).render(
  <StrictMode>
    <BrowserRouter basename="/admin">
      <I18nProvider>
        <AuthProvider>
          <App />
        </AuthProvider>
      </I18nProvider>
    </BrowserRouter>
  </StrictMode>,
);
