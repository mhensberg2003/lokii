import { useState, type CSSProperties } from "react";
import styles from "./Artwork.module.css";

type ArtworkProps = {
  src: string | null;
  alt: string;
  /** AniList accent color, used as the placeholder tint while the image loads. */
  color?: string | null;
  ratio: "poster" | "wide" | "banner";
  className?: string;
};

/** An image that fades in over a tinted placeholder, so rows never flash white. */
export function Artwork({ src, alt, color, ratio, className }: ArtworkProps) {
  const [loaded, setLoaded] = useState(false);
  const style = { "--tint": color ?? "var(--surface-2)" } as CSSProperties;
  return (
    <div className={[styles.frame, styles[ratio], className].filter(Boolean).join(" ")} style={style}>
      {src && (
        <img
          src={src}
          alt={alt}
          loading="lazy"
          decoding="async"
          draggable={false}
          className={styles.image}
          data-loaded={loaded}
          onLoad={() => setLoaded(true)}
        />
      )}
    </div>
  );
}
