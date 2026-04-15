import { useTranslation } from "../i18n";
import { NavLink } from "react-router-dom";

interface LayoutProps {
  children: React.ReactNode;
}

export default function Layout({ children }: LayoutProps) {
  const { t } = useTranslation();

  return (
    <div className="layout">
      <header className="header">
        <div className="header-left">
          <span className="logo">altgo</span>
          <span className="subtitle">{t("title.subtitle")}</span>
        </div>
        <nav className="header-nav">
          <NavLink
            to="/"
            end
            className={({ isActive }) =>
              `nav-link ${isActive ? "active" : ""}`
            }
          >
            {t("nav.home")}
          </NavLink>
          <NavLink
            to="/settings"
            className={({ isActive }) =>
              `nav-link ${isActive ? "active" : ""}`
            }
          >
            {t("nav.settings")}
          </NavLink>
        </nav>
      </header>
      <main className="main">{children}</main>
    </div>
  );
}
