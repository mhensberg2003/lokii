import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import { ChevronLeft, ChevronRight } from "lucide-react";
import styles from "./Row.module.css";

type RowProps = {
  title: string;
  children: ReactNode;
  /** Card width in px; the row scrolls by whole pages of cards. */
  itemWidth?: number;
};

/** A titled horizontal row of cards. Scrolls with trackpad, or by page with the arrow buttons. */
export function Row({ title, children, itemWidth = 168 }: RowProps) {
  const trackRef = useRef<HTMLDivElement>(null);
  const [canBack, setCanBack] = useState(false);
  const [canForward, setCanForward] = useState(false);

  const update = useCallback(() => {
    const el = trackRef.current;
    if (!el) return;
    setCanBack(el.scrollLeft > 4);
    setCanForward(el.scrollLeft + el.clientWidth < el.scrollWidth - 4);
  }, []);

  useEffect(() => {
    const el = trackRef.current;
    if (!el) return;
    update();
    const observer = new ResizeObserver(update);
    observer.observe(el);
    return () => observer.disconnect();
  }, [update, children]);

  const page = (direction: 1 | -1) => {
    const el = trackRef.current;
    if (!el) return;
    el.scrollBy({ left: direction * (el.clientWidth - itemWidth), behavior: "smooth" });
  };

  return (
    <section className={styles.row} aria-label={title}>
      <header className={styles.header}>
        <h2 className={styles.title}>{title}</h2>
        <div className={styles.arrows}>
          <button type="button" className={styles.arrow} onClick={() => page(-1)} disabled={!canBack} aria-label={`Scroll ${title} back`}>
            <ChevronLeft />
          </button>
          <button type="button" className={styles.arrow} onClick={() => page(1)} disabled={!canForward} aria-label={`Scroll ${title} forward`}>
            <ChevronRight />
          </button>
        </div>
      </header>
      <div
        ref={trackRef}
        className={styles.track}
        style={{ gridAutoColumns: `${itemWidth}px` }}
        onScroll={update}
      >
        {children}
      </div>
    </section>
  );
}
