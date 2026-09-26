import { useState, type FormEvent } from "react";
import { Button } from "../components/ui/Button";
import { errorText } from "../components/PageMessage";
import { maskedKey, useConnectTorBox, useDisconnectTorBox, useTorBoxStatus, type TorBoxStatus } from "../lib/settings";
import { Row, openLink } from "./SettingsRows";
import styles from "./Settings.module.css";

const KEY_PAGE = "https://torbox.app/settings";

/** The TorBox rows: a Connect form, or the connected account with its API key. */
export function TorBoxRows() {
  const status = useTorBoxStatus();
  if (status.isPending) {
    return <Row name="TorBox" description="Checking the connection…" />;
  }
  if (status.isError) {
    return (
      <Row name="TorBox" description={<span className={styles.error}>{errorText(status.error)}</span>}>
        <Button size="sm" onClick={() => status.refetch()}>
          Try again
        </Button>
      </Row>
    );
  }
  return status.data.connected ? <ConnectedRows status={status.data} /> : <ConnectRow />;
}

function ConnectRow() {
  const [open, setOpen] = useState(false);
  const [key, setKey] = useState("");
  const connect = useConnectTorBox();

  const submit = (event: FormEvent) => {
    event.preventDefault();
    connect.mutate(key, { onSuccess: () => setKey("") });
  };
  const cancel = () => {
    setOpen(false);
    setKey("");
    connect.reset();
  };

  const form = open && (
    <form className={styles.inlineForm} onSubmit={submit}>
      <input
        className={styles.input}
        type="password"
        value={key}
        onChange={(event) => setKey(event.target.value)}
        placeholder="Paste your TorBox API key"
        aria-label="TorBox API key"
        autoComplete="off"
        spellCheck={false}
        autoFocus
      />
      <Button type="submit" size="sm" variant="primary" disabled={!key.trim() || connect.isPending}>
        {connect.isPending ? "Connecting…" : "Connect"}
      </Button>
      <Button size="sm" variant="ghost" onClick={cancel}>
        Cancel
      </Button>
      {connect.isError && <p className={styles.error}>{errorText(connect.error)}</p>}
      <p className={styles.hint}>
        Find your API key on{" "}
        <button type="button" className={styles.link} onClick={() => openLink(KEY_PAGE)}>
          torbox.app/settings
        </button>
        . Lokii keeps it in the system keychain.
      </p>
    </form>
  );

  return (
    <Row
      name="TorBox"
      description="Streams from TorBox's servers. Lokii uses it whenever it is connected."
      below={form}
    >
      {!open && (
        <Button size="sm" onClick={() => setOpen(true)}>
          Connect
        </Button>
      )}
    </Row>
  );
}

function ConnectedRows({ status }: { status: TorBoxStatus }) {
  const account = status.account;
  const details = account ? [`${account.plan} plan`, account.email].filter(Boolean).join(" · ") : "Account details did not load.";
  return (
    <>
      <Row
        name="Account"
        description={details}
        below={status.error && <p className={styles.warning}>{status.error}</p>}
      >
        <span className={styles.status}>
          <span className={styles.dot} aria-hidden="true" />
          Connected
        </span>
      </Row>
      <KeyRow hint={status.keyHint} />
    </>
  );
}

function KeyRow({ hint }: { hint: string | null }) {
  const [confirming, setConfirming] = useState(false);
  const disconnect = useDisconnectTorBox();

  const confirm = (
    <div className={styles.inlineForm} role="group" aria-label="Disconnect TorBox">
      <span className={styles.hint}>Disconnect TorBox? New Streams then use Local Torrent.</span>
      <Button size="sm" variant="primary" disabled={disconnect.isPending} onClick={() => disconnect.mutate()}>
        Disconnect
      </Button>
      <Button size="sm" variant="ghost" onClick={() => setConfirming(false)}>
        Cancel
      </Button>
      {disconnect.isError && <p className={styles.error}>{errorText(disconnect.error)}</p>}
    </div>
  );

  return (
    <Row
      name="API key"
      description={<span className={styles.mono}>{maskedKey(hint)}</span>}
      below={confirming && confirm}
    >
      {!confirming && (
        <Button size="sm" variant="ghost" onClick={() => setConfirming(true)}>
          Disconnect
        </Button>
      )}
    </Row>
  );
}
