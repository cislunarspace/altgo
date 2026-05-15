import { useTranslation } from "../i18n";
import { NavLink } from "react-router-dom";
import { Mic, Settings, History, Minus, Maximize2, Minimize2, X } from "lucide-react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useState, useEffect } from "react";
import "../styles/components.css";

interface LayoutProps {
  children: React.ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  const { t } = useTranslation();
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    const win = getCurrentWindow();
    let unlisten: (() => void) | null = null;

    const checkMaximized = async () => {
      try {
        const maximized = await win.isMaximized();
        setIsMaximized(maximized);
      } catch {
        // ignore
      }
    };

    checkMaximized();

    win.onResized(() => {
      checkMaximized();
    }).then((fn) => {
      unlisten = fn;
    });

    return () => {
      if (unlisten) unlisten();
    };
  }, []);

  useEffect(() => {
    const root = document.getElementById("root");
    if (!root) return;
    root.classList.toggle("maximized", isMaximized);
  }, [isMaximized]);

  const handleMinimize = async () => {
    try {
      await getCurrentWindow().minimize();
    } catch {
      // ignore
    }
  };

  const handleToggleMaximize = async () => {
    try {
      await getCurrentWindow().toggleMaximize();
    } catch {
      // ignore
    }
  };

  const handleClose = async () => {
    try {
      await getCurrentWindow().hide();
    } catch {
      // ignore
    }
  };

  const handleHeaderMouseDown = (e: React.MouseEvent) => {
    const target = e.target as HTMLElement;
    if (
      target.closest("a") ||
      target.closest("button") ||
      target.closest("nav") ||
      target.closest(".window-controls")
    ) {
      return;
    }
    try {
      getCurrentWindow().startDragging();
    } catch {
      // ignore
    }
  };

  return (
    <div className="layout">
      <header className="layout-header" data-tauri-drag-region onMouseDown={handleHeaderMouseDown}>
        <div className="layout-header-left">
          <div className="layout-logo-wrapper">
            <img
              src="/altgo-logo.svg"
              alt=""
              width={28}
              height={28}
              className="layout-logo-mark"
            />
            <span className="layout-logo">altgo</span>
          </div>
          <span className="layout-subtitle">{t("title.subtitle")}</span>
        </div>
        <div className="layout-header-right" data-tauri-drag-region="false">
          <nav className="layout-nav">
            <NavLink
              to="/"
              end
              className={({ isActive }) =>
                `layout-nav-link ${isActive ? "active" : ""}`
              }
            >
              <Mic size={16} />
              {t("nav.home")}
            </NavLink>
            <NavLink
              to="/history"
              className={({ isActive }) =>
                `layout-nav-link ${isActive ? "active" : ""}`
              }
            >
              <History size={16} />
              {t("nav.history")}
            </NavLink>
            <NavLink
              to="/settings"
              className={({ isActive }) =>
                `layout-nav-link ${isActive ? "active" : ""}`
              }
            >
              <Settings size={16} />
              {t("nav.settings")}
            </NavLink>
          </nav>
          <div className="window-controls">
            <button
              className="window-control-btn"
              onClick={handleMinimize}
              aria-label="Minimize"
            >
              <Minus size={14} />
            </button>
            <button
              className="window-control-btn"
              onClick={handleToggleMaximize}
              aria-label={isMaximized ? "Restore" : "Maximize"}
            >
              {isMaximized ? <Minimize2 size={14} /> : <Maximize2 size={14} />}
            </button>
            <button
              className="window-control-btn window-control-close"
              onClick={handleClose}
              aria-label="Close"
            >
              <X size={14} />
            </button>
          </div>
        </div>
      </header>
      <main className="layout-main">{children}</main>
    </div>
  );
}
