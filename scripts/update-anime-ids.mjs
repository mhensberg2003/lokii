// Rebuilds src-tauri/src/ids/anime-ids.tsv from the Fribb anime-lists mapping.
// The app ships this file and refreshes the same data at runtime every week.
// Usage: node scripts/update-anime-ids.mjs
import { writeFileSync } from "node:fs";

const SOURCE = "https://raw.githubusercontent.com/Fribb/anime-lists/master/anime-list-mini.json";
const TARGET = new URL("../src-tauri/src/ids/anime-ids.tsv", import.meta.url);

const response = await fetch(SOURCE);
if (!response.ok) throw new Error(`Fribb anime-lists returned HTTP ${response.status}`);
const entries = await response.json();

const byAniList = new Map();
for (const { anilist_id: anilist, anidb_id: anidb, mal_id: mal } of entries) {
  if (!Number.isInteger(anilist) || (!Number.isInteger(anidb) && !Number.isInteger(mal))) continue;
  if (!byAniList.has(anilist)) byAniList.set(anilist, [anidb ?? "", mal ?? ""]);
}

const lines = [...byAniList.entries()]
  .sort(([a], [b]) => a - b)
  .map(([anilist, [anidb, mal]]) => `${anilist}\t${anidb}\t${mal}`);
writeFileSync(TARGET, `anilist\tanidb\tmal\n${lines.join("\n")}\n`);
process.stdout.write(`Wrote ${lines.length} rows to ${TARGET.pathname}\n`);
