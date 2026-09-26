import { useMutation, useQuery } from "@tanstack/react-query";
import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { ArrowUpCircle } from "lucide-react";
import { Button } from "../components/ui/Button";
import styles from "./UpdateNotice.module.css";

/** Dev builds and the browser preview have no release to update from. */
const CHECKS_FOR_UPDATES = !import.meta.env.DEV && import.meta.env.VITE_PREVIEW !== "1";

/**
 * Asks GitHub Releases for a newer Lokii once per start. When there is one, a button
 * downloads it, installs it and restarts the app.
 */
export function UpdateNotice({ enabled = CHECKS_FOR_UPDATES }: { enabled?: boolean }) {
  const update = useQuery({
    queryKey: ["update"],
    queryFn: () => check(),
    enabled,
    staleTime: Infinity,
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
