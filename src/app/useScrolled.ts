import { useEffect, useState } from "react";

/** True once the element with this id has scrolled past `threshold` px. */
export function useScrolled(elementId: string, threshold = 24): boolean {
  const [scrolled, setScrolled] = useState(false);

  useEffect(() => {
    const el = document.getElementById(elementId);
    if (!el) return;
    const onScroll = () => setScrolled(el.scrollTop > threshold);
    onScroll();
    el.addEventListener("scroll", onScroll, { passive: true });
    return () => el.removeEventListener("scroll", onScroll);
  }, [elementId, threshold]);

  return scrolled;
}
