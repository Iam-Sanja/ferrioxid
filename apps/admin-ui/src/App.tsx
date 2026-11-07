import type { ReactElement } from "react";
import { Navigate, Route, Routes } from "react-router-dom";
import { useAuth } from "./auth/AuthProvider";
import { LoginPage } from "./auth/LoginPage";
import { Layout } from "./components/Layout";
import { Dashboard } from "./pages/Dashboard";
import { Directory } from "./pages/Directory";
import { ModelCreate } from "./pages/ModelCreate";
import { ModelDetail } from "./pages/ModelDetail";
import { ModelEdit } from "./pages/ModelEdit";
import { ModelList } from "./pages/ModelList";
import { useI18n } from "./i18n";
import { Forbidden, NotFound } from "./pages/StatusPages";

function Guard({ children }: { children: ReactElement }) {
  const { user, ready } = useAuth();
  const { t } = useI18n();
  if (!ready) return <p role="status">{t("auth.session")}</p>;
  if (!user) return <Navigate to="/login" replace />;
  return children;
}

export function App() {
  return (
    <Routes>
      <Route path="/login" element={<LoginPage />} />
      <Route path="/forbidden" element={<Forbidden />} />
      <Route path="/" element={<Guard><Layout><Dashboard /></Layout></Guard>} />
      <Route path="/directory" element={<Guard><Layout><Directory /></Layout></Guard>} />
      <Route path="/models/:model" element={<Guard><Layout><ModelList /></Layout></Guard>} />
      <Route path="/models/:model/new" element={<Guard><Layout><ModelCreate /></Layout></Guard>} />
      <Route path="/models/:model/:id" element={<Guard><Layout><ModelDetail /></Layout></Guard>} />
      <Route path="/models/:model/:id/edit" element={<Guard><Layout><ModelEdit /></Layout></Guard>} />
      <Route path="*" element={<NotFound />} />
    </Routes>
  );
}
