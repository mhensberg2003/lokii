import { useEffect, useRef, useState } from "react";
import { useLocation, useNavigate, useSearchParams } from "react-router";
import { ChevronLeft, ChevronRight, Search, X } from "lucide-react";
import { useScrolled } from "./useScrolled";
import styles from "./TopBar.module.css";

const IS_MAC = typeof navigator !== "undefined" && /Mac/.test(navigator.userAgent);
const SEARCH_DELAY_MS = 250;

export function TopBar() {
  const navigate = useNavigate();
  const { pathname } = useLocation();
  const [params] = useSearchParams();
  const inputRef = useRef<HTMLInputElement>(null);
  const onSearchPage = pathname === "/search";
  const [query, setQuery] = useState(onSearchPage ? (params.get("q") ?? "") : "");
  const scrolled = useScrolled("content");

  // Cmd/Ctrl+K focuses search from anywhere.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        inputRef.current?.focus();
        inputRef.current?.select();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // Leaving the search page clears the field.
  useEffect(() => {
    if (!onSearchPage) setQuery("");
  }, [onSearchPage]);

  // Typing updates the search page after a short pause.
  useEffect(() => {
    const trimmed = query.trim();
    if (!trimmed) return;
    const timer = window.setTimeout(() => {
      navigate(`/search?q=${encodeURIComponent(trimmed)}`, { replace: onSearchPage });
    }, SEARCH_DELAY_MS);
    return () => window.clearTimeout(timer);
  }, [query]); // eslint-disable-line react-hooks/exhaustive-deps

  const clear = () => {
    setQuery("");
    if (onSearchPage) navigate(-1);
  };

  return (
    <header className={styles.bar} data-scrolled={scrolled} data-tauri-drag-region>
      <div className={styles.history}>
        <button type="button" className={styles.historyButton} onClick={() => navigate(-1)} aria-label="Back">
          <ChevronLeft />
        </button>
        <button type="button" className={styles.historyButton} onClick={() => navigate(1)} aria-label="Forward">
          <ChevronRight />
        </button>
      </div>

      <label className={styles.search}>
        <Search className={styles.searchIcon} aria-hidden="true" />
        <input
          ref={inputRef}
          id="global-search"
          type="search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              clear();
              inputRef.current?.blur();
            }
          }}
          placeholder="Search anime"
          autoComplete="off"
          spellCheck={false}
          aria-label="Search anime"
        />
        {query ? (
          <button type="button" className={styles.clear} onClick={clear} aria-label="Clear search">
            <X />
          </button>
        ) : (
          <kbd className={styles.shortcut}>{IS_MAC ? "⌘K" : "Ctrl K"}</kbd>
        )}
      </label>
    </header>
  );
}
