import { useTranslation } from "../i18n";
import { NavLink } from "react-router-dom";
import { Mic, Settings } from "lucide-react";
import "../styles/components.css";

interface LayoutProps {
  children: React.ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  const { t } = useTranslation();

  return (
    <div className="layout">
      <header className="layout-header">
        <div className="layout-header-left">
          <div className="layout-logo-wrapper">
            <Mic size={20} className="layout-logo-icon" />
            <span className="layout-logo">altgo</span>
          </div>
          <span className="layout-subtitle">{t("title.subtitle")}</span>
        </div>
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
            to="/settings"
            className={({ isActive }) =>
              `layout-nav-link ${isActive ? "active" : ""}`
            }
          >
            <Settings size={16} />
            {t("nav.settings")}
          </NavLink>
        </nav>
      </header>
      <main className="layout-main">{children}</main>
    </div>
  );
}
