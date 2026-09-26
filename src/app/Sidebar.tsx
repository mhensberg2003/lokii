import { NavLink } from "react-router";
import { ArrowDownToLine, Compass, House, LibraryBig, Settings, type LucideIcon } from "lucide-react";
import styles from "./Sidebar.module.css";

type NavItem = { to: string; label: string; icon: LucideIcon; end?: boolean };

const MAIN_NAV: NavItem[] = [
  { to: "/", label: "Home", icon: House, end: true },
  { to: "/browse", label: "Browse", icon: Compass },
  { to: "/library", label: "Library", icon: LibraryBig },
  { to: "/downloads", label: "Downloads", icon: ArrowDownToLine },
];

const SETTINGS: NavItem = { to: "/settings", label: "Settings", icon: Settings };

function Item({ to, label, icon: Icon, end }: NavItem) {
  return (
    <NavLink to={to} end={end} className={({ isActive }) => (isActive ? `${styles.item} ${styles.active}` : styles.item)}>
      <Icon className={styles.icon} strokeWidth={1.75} />
      <span>{label}</span>
    </NavLink>
  );
}

export function Sidebar() {
  return (
    <aside className={styles.sidebar}>
      <div className={styles.brand} data-tauri-drag-region>
        <span className={styles.wordmark}>Lokii</span>
      </div>
      <nav className={styles.nav} aria-label="Main">
        {MAIN_NAV.map((item) => (
          <Item key={item.to} {...item} />
        ))}
      </nav>
      <nav className={styles.footer} aria-label="App">
        <Item {...SETTINGS} />
      </nav>
    </aside>
  );
}
