import { createContext, useContext, useEffect, useState, type ReactNode } from "react";
import { adminMe, logout, restoreSession } from "../api/client";
import type { PublicUser } from "../types";

type AuthState = {
  user: PublicUser | null;
  ready: boolean;
  setUser: (user: PublicUser | null) => void;
  signOut: () => Promise<void>;
};

const AuthContext = createContext<AuthState | null>(null);

export function AuthProvider({ children }: { children: ReactNode }) {
  const [user, setUser] = useState<PublicUser | null>(null);
  const [ready, setReady] = useState(false);

  useEffect(() => {
    restoreSession()
      .then(async (restored) => {
        if (!restored?.is_staff) {
          setUser(null);
          return;
        }
        setUser(await adminMe());
      })
      .catch(() => setUser(null))
      .finally(() => setReady(true));
  }, []);

  return (
    <AuthContext.Provider
      value={{
        user,
        ready,
        setUser,
        signOut: async () => {
          await logout();
          setUser(null);
        },
      }}
    >
      {children}
    </AuthContext.Provider>
  );
}

export function useAuth(): AuthState {
  const value = useContext(AuthContext);
  if (!value) {
    throw new Error("AuthProvider fehlt");
  }
  return value;
}
