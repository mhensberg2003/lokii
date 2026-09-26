import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";
import { Info, Plug } from "lucide-react";
import { TorBoxRows } from "./SettingsTorBox";
import { LocalTorrentRows } from "./SettingsLocal";
import { Group, Row, openLink } from "./SettingsRows";
import styles from "./Settings.module.css";

type Section = "sources" | "about";

const SECTIONS: { id: Section; label: string; icon: typeof Plug }[] = [
  { id: "sources", label: "Sources", icon: Plug },
  { id: "about", label: "About", icon: Info },
];

/** Settings in the Cursor style: a section list on the left, grouped rows on the right. */
export function Settings() {
  const [section, setSection] = useState<Section>("sources");
  return (
    <div className={styles.page}>
      <h1 className={styles.heading}>Settings</h1>
      <div className={styles.layout}>
        <nav className={styles.nav} aria-label="Settings sections">
          {SECTIONS.map(({ id, label, icon: Icon }) => (
            <button
              key={id}
              type="button"
              className={styles.navItem}
              aria-current={section === id ? "page" : undefined}
              onClick={() => setSection(id)}
            >
              <Icon aria-hidden="true" />
              {label}
            </button>
          ))}
        </nav>
        <div className={styles.sections}>{section === "sources" ? <SourcesSection /> : <AboutSection />}</div>
      </div>
    </div>
  );
}

function SourcesSection() {
  return (
    <>
      <Group title="TorBox" description="Lokii streams from TorBox when you connect an account.">
        <TorBoxRows />
      </Group>
      <Group
        title="Local Torrent"
        description="When TorBox is not connected, this device downloads and seeds the Release itself."
        note="Lokii deletes this data when you quit, and after you watch an Episode."
      >
        <LocalTorrentRows />
      </Group>
    </>
  );
}

function AboutSection() {
  const version = useQuery({ queryKey: ["app", "version"], queryFn: getVersion, staleTime: Infinity });
  return (
    <Group title="About">
      <Row name="Version" description="The Lokii build on this device.">
        <span className={styles.value}>{version.data ?? "–"}</span>
      </Row>
      <Row name="License" description="Lokii is open source. You can read and change the code.">
        <button
          type="button"
          className={styles.link}
          onClick={() => openLink("https://github.com/mhensberg2003/lokii")}
        >
          GPL-3.0-or-later
        </button>
      </Row>
    </Group>
  );
}
