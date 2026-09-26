import { ArrowDownToLine, LibraryBig, Settings as SettingsIcon } from "lucide-react";
import { PageMessage } from "../components/PageMessage";

export function Library() {
  return (
    <PageMessage icon={LibraryBig} title="Your Library">
      Your Watchlist and Continue watching will appear here.
    </PageMessage>
  );
}

export function Downloads() {
  return (
    <PageMessage icon={ArrowDownToLine} title="No downloads">
      Episodes that stream through TorBox or a local torrent will appear here.
    </PageMessage>
  );
}

export function Settings() {
  return (
    <PageMessage icon={SettingsIcon} title="Settings">
      Connect TorBox and set up local torrents here.
    </PageMessage>
  );
}
