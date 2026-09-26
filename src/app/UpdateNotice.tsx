import { useMutation, useQuery } from "@tanstack/react-query";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ArrowUpCircle } from "lucide-react";
import { Button } from "../components/ui/Button";
import styles from "./UpdateNotice.module.css";

/** Dev builds and the browser preview have no release to update from. */
const CHECKS_FOR_UPDATES = !import.meta.env.DEV && import.meta.env.VITE_PREVIEW !== "1";

/** Lokii can stay open for days, so it asks again after this time. */
const CHECK_EVERY = 6 * 60 * 60 * 1000;

/**
 * Asks GitHub Releases for a newer Lokii at start and every few hours. When there is one, a button
 * downloads it, installs it and restarts the app.
 */
export function UpdateNotice({ enabled = CHECKS_FOR_UPDATES }: { enabled?: boolean }) {
  const update = useQuery({
    queryKey: ["update"],
    queryFn: () => check(),
    enabled,
    staleTime: CHECK_EVERY,
    refetchInterval: CHECK_EVERY,
    retry: false,
  });
  const install = useMutation({
    mutationFn: async () => {
      await update.data?.downloadAndInstall();
      await relaunch();
    },
  });

  if (!update.data) return null;
  return (
    <div className={styles.notice}>
      <Button
        variant="primary"
        size="sm"
        icon={<ArrowUpCircle size={16} />}
        disabled={install.isPending}
        onClick={() => install.mutate()}
      >
        {install.isPending ? "Updating…" : `Update to ${update.data.version}`}
      </Button>
      {install.isError && <span className={styles.error}>The update failed. Try again later.</span>}
    </div>
  );
}
