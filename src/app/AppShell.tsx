import { Outlet, useLocation } from "react-router";
import { useEffect, useRef } from "react";
import { Sidebar } from "./Sidebar";
import { TopBar } from "./TopBar";
import { ActivityPanel } from "./ActivityPanel";
import styles from "./AppShell.module.css";

/** Persistent sidebar and top bar. Only the content area scrolls. */
export function AppShell() {
  const scrollRef = useRef<HTMLDivElement>(null);
  const { pathname } = useLocation();

  // Each new page starts at the top.
  useEffect(() => {
    scrollRef.current?.scrollTo({ top: 0 });
  }, [pathname]);

  return (
    <div className={styles.shell}>
      <Sidebar />
      <div className={styles.main}>
        <TopBar />
        <div ref={scrollRef} className={styles.content} id="content">
          <Outlet />
        </div>
        <ActivityPanel />
      </div>
    </div>
  );
}
