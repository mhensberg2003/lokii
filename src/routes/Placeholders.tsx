import { LibraryBig } from "lucide-react";
import { PageMessage } from "../components/PageMessage";

export function Library() {
  return (
    <PageMessage icon={LibraryBig} title="Your Library">
      Your Watchlist and Continue watching will appear here.
    </PageMessage>
  );
}
