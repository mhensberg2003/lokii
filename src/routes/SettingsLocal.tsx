import { useEffect, useState, type KeyboardEvent } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "../components/ui/Button";
import { errorText } from "../components/PageMessage";
import { parsePort, useLocalSettings, useSetLocalSettings, type LocalSettings } from "../lib/settings";
import { Row } from "./SettingsRows";
import styles from "./Settings.module.css";

/** The Local Torrent rows: the listening port and the download folder. */
export function LocalTorrentRows() {
  const local = useLocalSettings();
  if (local.isPending) return <Row name="Port" description="Loading…" />;
  if (local.isError) {
    return (
      <Row name="Local Torrent" description={<span className={styles.error}>{errorText(local.error)}</span>}>
        <Button size="sm" onClick={() => local.refetch()}>
          Try again
        </Button>
      </Row>
    );
  }
  return (
    <>
      <PortRow value={local.data} />
      <FolderRow value={local.data} />
      {local.data.restartNeeded && <p className={styles.warning}>Restart Lokii to apply these changes.</p>}
    </>
  );
}

function PortRow({ value }: { value: LocalSettings }) {
  const save = useSetLocalSettings();
  const [text, setText] = useState(value.port?.toString() ?? "");
  const [error, setError] = useState<string | null>(null);

  // Show the saved value again when it changes somewhere else.
  useEffect(() => setText(value.port?.toString() ?? ""), [value.port]);

  const commit = () => {
    const parsed = parsePort(text);
    if ("error" in parsed) return setError(parsed.error);
    setError(null);
    if (parsed.port === value.port) return;
    save.mutate({ port: parsed.port, folder: value.folder }, { onError: (err) => setError(errorText(err)) });
  };
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Enter") event.currentTarget.blur();
  };

  return (
    <Row
      name="Port"
      description="The port other peers use to connect. Leave it empty to let the system choose."
      below={error && <p className={styles.error}>{error}</p>}
    >
      <input
        className={`${styles.input} ${styles.portInput}`}
        inputMode="numeric"
        value={text}
        onChange={(event) => setText(event.target.value)}
        onBlur={commit}
        onKeyDown={onKeyDown}
        placeholder="Automatic"
        aria-label="Port"
        aria-invalid={error !== null}
      />
    </Row>
  );
}

function FolderRow({ value }: { value: LocalSettings }) {
  const save = useSetLocalSettings();
  const setFolder = (folder: string | null) => save.mutate({ port: value.port, folder });
  const choose = async () => {
    const picked = await open({ directory: true, title: "Choose a download folder" });
    if (typeof picked === "string") setFolder(picked);
  };

  return (
    <Row
      name="Download folder"
      description={
        <span className={styles.path} title={value.dataDir}>
          {value.dataDir}
        </span>
      }
      below={save.isError && <p className={styles.error}>{errorText(save.error)}</p>}
    >
      {value.folder && (
        <Button size="sm" variant="ghost" disabled={save.isPending} onClick={() => setFolder(null)}>
          Use default
        </Button>
      )}
      <Button size="sm" disabled={save.isPending} onClick={choose}>
        Change
      </Button>
    </Row>
  );
}
