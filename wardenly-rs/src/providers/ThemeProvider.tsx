import { useEffect, useState, createContext, useContext, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/core";

interface ThemeResponse {
  activeTheme: string;
  cssVars: Record<string, string>;
  availableThemes: string[];
}

interface ThemeContextValue {
  activeTheme: string;
  availableThemes: string[];
  isLoading: boolean;
}

const ThemeContext = createContext<ThemeContextValue>({
  activeTheme: "ocean-dark",
  availableThemes: [],
  isLoading: true,
});

export function useTheme() {
  return useContext(ThemeContext);
}

interface ThemeProviderProps {
  children: ReactNode;
}

export function ThemeProvider({ children }: ThemeProviderProps) {
  const [activeTheme, setActiveTheme] = useState("ocean-dark");
  const [availableThemes, setAvailableThemes] = useState<string[]>([]);
  const [isLoading, setIsLoading] = useState(true);

  useEffect(() => {
    const loadTheme = async () => {
      try {
        const response = await invoke<ThemeResponse>("get_theme_config");
        
        // Inject CSS variables into :root
        const root = document.documentElement;
        for (const [varName, value] of Object.entries(response.cssVars)) {
          root.style.setProperty(varName, value);
        }

        setActiveTheme(response.activeTheme);
        setAvailableThemes(response.availableThemes);
      } catch (error) {
        console.error("Failed to load theme config:", error);
        // Theme will use CSS fallback values defined in globals.css
      } finally {
        setIsLoading(false);
      }
    };

    loadTheme();
  }, []);

  return (
    <ThemeContext.Provider value={{ activeTheme, availableThemes, isLoading }}>
      {children}
    </ThemeContext.Provider>
  );
}

