"use client";

import { createContext, useCallback, useContext, useEffect, useState, type ReactNode } from "react";
import { usePathname } from "next/navigation";
import { clearToken, getMe, getToken, type UserDto } from "./api";

type Session = {
  /** undefined while the first check runs, null when signed out. */
  me: UserDto | null | undefined;
  refresh: () => void;
  logout: () => void;
};

const Ctx = createContext<Session>({ me: undefined, refresh: () => {}, logout: () => {} });

/** One place that knows who is signed in, so the top bar, tab bar and pages agree. */
export function SessionProvider({ children }: { children: ReactNode }) {
  const [me, setMe] = useState<UserDto | null | undefined>(undefined);
  const pathname = usePathname();

  const refresh = useCallback(() => {
    if (!getToken()) {
      setMe(null);
      return;
    }
    getMe()
      .then(setMe)
      .catch((e: unknown) => {
        // Only a rejected token signs out; offline or a 5xx keeps the session (door staff on bad signal).
        if (e instanceof Error && e.message === "Потрібен вхід") {
          clearToken();
          setMe(null);
        }
      });
  }, []);

  // First load, and every navigation where the stored token and the known user disagree (a login page
  // stores the token and then routes away; a 401 elsewhere clears it).
  useEffect(() => {
    if (me === undefined || !!getToken() !== !!me) refresh();
  }, [pathname, me, refresh]);

  const logout = useCallback(() => {
    clearToken();
    setMe(null);
  }, []);

  return <Ctx.Provider value={{ me, refresh, logout }}>{children}</Ctx.Provider>;
}

export function useSession(): Session {
  return useContext(Ctx);
}
