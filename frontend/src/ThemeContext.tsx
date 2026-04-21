import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  applyThemeToDocument,
  getThemePref,
  installThemeListeners,
  resolveEffectiveTheme,
  setThemePref,
  type ThemePref,
} from "./theme";

export type { ThemePref };

type ThemeContextValue = {
  themePref: ThemePref;
  effectiveTheme: "light" | "dark";
  setTheme: (next: ThemePref) => void;
};

const ThemeContext = createContext<ThemeContextValue | null>(null);

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [pref, setPref] = useState<ThemePref>(() => getThemePref());

  useEffect(() => {
    return installThemeListeners(() => {
      const p = getThemePref();
      setPref(p);
      applyThemeToDocument(p);
    });
  }, []);

  const setTheme = useCallback((next: ThemePref) => {
    setPref(next);
    setThemePref(next);
  }, []);

  const value = useMemo<ThemeContextValue>(
    () => ({
      themePref: pref,
      effectiveTheme: resolveEffectiveTheme(pref),
      setTheme,
    }),
    [pref, setTheme],
  );

  return <ThemeContext.Provider value={value}>{children}</ThemeContext.Provider>;
}

export function useTheme(): ThemeContextValue {
  const ctx = useContext(ThemeContext);
  if (!ctx) {
    throw new Error("useTheme must be used within ThemeProvider");
  }
  return ctx;
}
