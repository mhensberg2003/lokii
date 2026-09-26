//! Downloads the inputs of the One Pace mapping: the Episode Guide and Episode
//! Descriptions sheets (Google Sheets CSV export), the Arc posters (the
//! one-pace-metadata project on GitHub), and the "[One Pace]" Releases on Nyaa.

use std::collections::{BTreeMap, HashSet};
use std::time::Duration;

use regex::Regex;
use serde::Deserialize;

use super::build::ListedRelease;
use super::model::{NyaaRelease, ReleaseFileInfo};

const GUIDE_SHEET: &str = "1HQRMJgu_zArp-sLnvFMDzOyjdsht87eFLECxMK858lA";
const DESCRIPTIONS_SHEET: &str = "1M0Aa2p5x7NioaH9-u8FyHq6rH3t5s6Sccs8GoC6pHAM";
const EPISODE_DESCRIPTIONS_TAB: &str = "0";
const ARC_DESCRIPTIONS_TAB: &str = "2010244982";
const POSTER_ARCS_URL: &str =
    "https://raw.githubusercontent.com/ladyisatis/one-pace-metadata/refs/heads/v2/metadata/arcs.min.json";
const POSTER_TREE_URL: &str = "https://api.github.com/repos/ladyisatis/one-pace-metadata/git/trees/v2?recursive=1";
/// The posters are Git LFS files, which raw.githubusercontent.com serves as pointers.
const POSTER_BASE: &str = "https://media.githubusercontent.com/media/ladyisatis/one-pace-metadata/v2/arcs/en";
const NYAA: &str = "https://nyaa.si";
/// Nyaa's search matches every word, so this finds every "one pace {arc}" Release.
const NYAA_QUERY: &str = "one pace";
const NYAA_PAGE_SIZE: usize = 75;
const NYAA_MAX_PAGES: u32 = 40;
/// Time between Nyaa requests, so a refresh never looks like a flood.
const NYAA_PAUSE: Duration = Duration::from_millis(700);

async fn get(http: &reqwest::Client, url: &str) -> Result<String, String> {
    let response = http.get(url).send().await.map_err(|e| format!("cannot reach {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("{url} answered HTTP {}", response.status()));
    }
    response.text().await.map_err(|e| format!("cannot read {url}: {e}"))
}

async fn sheet_csv(http: &reqwest::Client, sheet: &str, gid: &str) -> Result<String, String> {
    get(http, &format!("https://docs.google.com/spreadsheets/d/{sheet}/export?format=csv&gid={gid}")).await
}

/// Every tab of the Episode Guide in sheet order: (tab name, CSV).
pub async fn guide(http: &reqwest::Client) -> Result<Vec<(String, String)>, String> {
    let page = get(http, &format!("https://docs.google.com/spreadsheets/d/{GUIDE_SHEET}/htmlview")).await?;
    let tabs = guide_tabs(&page);
    if tabs.is_empty() {
        return Err("the Episode Guide lists no tabs".to_string());
    }
    let mut out = Vec::with_capacity(tabs.len());
    for (name, gid) in tabs {
        out.push((name, sheet_csv(http, GUIDE_SHEET, &gid).await?));
    }
    Ok(out)
}

/// (tab name, gid) pairs from the sheet's HTML view.
fn guide_tabs(page: &str) -> Vec<(String, String)> {
    let tab = Regex::new(r#"\{name: "((?:[^"\\]|\\.)*)", pageUrl: "[^"]*?gid=(\d+)"#).expect("valid regex");
    tab.captures_iter(page).map(|c| (unescape_js(&c[1]), c[2].to_string())).collect()
}

/// Undoes JavaScript string escapes: `\x27`, `&`, `\/`, `\"`.
fn unescape_js(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let width = match chars.clone().next() {
            Some('x') => 2,
            Some('u') => 4,
            _ => 0,
        };
        if width == 0 {
            out.extend(chars.next());
            continue;
        }
        chars.next();
        let hex: String = chars.by_ref().take(width).collect();
        out.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
    }
    out
}

/// The Episode Descriptions sheet: (Episodes tab, Arcs tab) as CSV.
pub async fn descriptions(http: &reqwest::Client) -> Result<(String, String), String> {
    let episodes = sheet_csv(http, DESCRIPTIONS_SHEET, EPISODE_DESCRIPTIONS_TAB).await?;
    let arcs = sheet_csv(http, DESCRIPTIONS_SHEET, ARC_DESCRIPTIONS_TAB).await?;
    Ok((episodes, arcs))
}

#[derive(Deserialize)]
struct PosterArcs {
    en: Vec<PosterArc>,
}

#[derive(Deserialize)]
struct PosterArc {
    part: i64,
    title: String,
}

#[derive(Deserialize)]
struct Tree {
    tree: Vec<TreeEntry>,
}

#[derive(Deserialize)]
struct TreeEntry {
    path: String,
}

/// (Arc title, poster URL) for every Arc that has a poster.
pub async fn posters(http: &reqwest::Client) -> Result<Vec<(String, String)>, String> {
    let arcs: PosterArcs = serde_json::from_str(&get(http, POSTER_ARCS_URL).await?).map_err(|e| e.to_string())?;
    let tree: Tree = serde_json::from_str(&get(http, POSTER_TREE_URL).await?).map_err(|e| e.to_string())?;
    let present: HashSet<String> = tree.tree.into_iter().map(|entry| entry.path).collect();
    Ok(arcs
        .en
        .into_iter()
        .filter(|arc| present.contains(&format!("arcs/en/{}/poster.png", arc.part)))
        .map(|arc| (arc.title, format!("{POSTER_BASE}/{}/poster.png", arc.part)))
        .collect())
}

/// Every "[One Pace]" Release on Nyaa with its file list, except for a Release named
/// like its one file. Lists in `known` (by Nyaa ID) are reused; a Release whose list
/// cannot load counts as one file named like the Release.
pub async fn nyaa_releases(
    http: &reqwest::Client,
    known: &BTreeMap<u64, Vec<ReleaseFileInfo>>,
) -> Result<Vec<ListedRelease>, String> {
    let rows = NyaaRows::new();
    let mut releases: BTreeMap<u64, NyaaRelease> = BTreeMap::new();
    for page in 1..=NYAA_MAX_PAGES {
        let url = format!("{NYAA}/?f=0&c=0_0&q={}&p={page}", NYAA_QUERY.replace(' ', "+"));
        let found = rows.parse(&get(http, &url).await?);
        let count = found.len();
        releases.extend(found.into_iter().map(|r| (r.nyaa_id, r)));
        if count < NYAA_PAGE_SIZE {
            break;
        }
        tokio::time::sleep(NYAA_PAUSE).await;
    }
    if releases.is_empty() {
        return Err("Nyaa listed no One Pace Releases".to_string());
    }

    let mut listed = Vec::new();
    for mut release in releases.into_values().filter(|r| r.title.to_lowercase().starts_with("[one pace]")) {
        let files = if let Some(files) = known.get(&release.nyaa_id) {
            Some(files.clone())
        } else if is_single_file(&release.title) {
            None
        } else {
            // Also one-file Releases, whose file name can differ from the title.
            tokio::time::sleep(NYAA_PAUSE).await;
            let page = get(http, &format!("{NYAA}/view/{}", release.nyaa_id)).await;
            page.ok().map(|page| file_list(&page)).filter(|files| !files.is_empty())
        };
        release.file_count = files.as_ref().map_or(1, |files| files.len() as u32);
        listed.push(ListedRelease { release, files });
    }
    Ok(listed)
}

/// A Release named like one file with its CRC32: "[One Pace][1000] Wano 55 [1080p][2501AC5A].mkv".
fn is_single_file(title: &str) -> bool {
    Regex::new(r"(?i)\[[0-9a-f]{8}\]\.(mkv|mp4)$").expect("valid regex").is_match(title)
}

/// Reads Nyaa's result table.
struct NyaaRows {
    row: Regex,
    view: Regex,
    magnet: Regex,
    hash: Regex,
    size: Regex,
    timestamp: Regex,
    number: Regex,
}

impl NyaaRows {
    fn new() -> Self {
        let regex = |pattern: &str| Regex::new(pattern).expect("valid regex");
        Self {
            row: regex(r#"(?s)<tr class="\w+">(.*?)</tr>"#),
            view: regex(r#"<a href="/view/(\d+)" title="([^"]*)""#),
            magnet: regex(r#"href="(magnet:[^"]+)""#),
            hash: regex(r"btih:([0-9a-fA-F]{40})"),
            size: regex(r#"<td class="text-center">([\d.]+) (\w+)</td>"#),
            timestamp: regex(r#"data-timestamp="(\d+)""#),
            number: regex(r#"<td class="text-center">(\d+)</td>"#),
        }
    }

    fn parse(&self, page: &str) -> Vec<NyaaRelease> {
        self.row.captures_iter(page).filter_map(|row| self.release(&row[1])).collect()
    }

    fn release(&self, row: &str) -> Option<NyaaRelease> {
        let view = self.view.captures(row)?;
        let magnet = decode_html(&self.magnet.captures(row)?[1]);
        let info_hash = self.hash.captures(&magnet)?[1].to_lowercase();
        let size = self.size.captures(row)?;
        let numbers: Vec<u32> = self.number.captures_iter(row).filter_map(|c| c[1].parse().ok()).collect();
        Some(NyaaRelease {
            info_hash,
            nyaa_id: view[1].parse().ok()?,
            title: decode_html(&view[2]),
            magnet,
            size_bytes: size_bytes(&size[1], &size[2]),
            seeders: *numbers.first()?,
            leechers: *numbers.get(1)?,
            published_at: self.timestamp.captures(row)?[1].parse().ok()?,
            file_count: 1,
        })
    }
}

/// The file list on a Nyaa Release page, with folders joined by `/`.
fn file_list(page: &str) -> Vec<ReleaseFileInfo> {
    let block = Regex::new(r#"(?s)torrent-file-list panel-body">(.*?)</div>"#).expect("valid regex");
    let token = Regex::new(
        r#"<a href="" class="folder">(?:<i[^>]*></i>)?([^<]*)</a>|<li><i class="fa fa-file"></i>([^<]*)<span class="file-size">\(([\d.]+) (\w+)\)</span>|</ul>"#,
    )
    .expect("valid regex");
    let Some(block) = block.captures(page) else { return Vec::new() };
    let (mut folders, mut files) = (Vec::new(), Vec::new());
    for found in token.captures_iter(&block[1]) {
        if let Some(folder) = found.get(1) {
            folders.push(decode_html(folder.as_str().trim()));
        } else if let Some(name) = found.get(2) {
            let mut path = folders.clone();
            path.push(decode_html(name.as_str().trim()));
            files.push(ReleaseFileInfo { path: path.join("/"), size: size_bytes(&found[3], &found[4]) });
        } else {
            folders.pop();
        }
    }
    files
}

fn size_bytes(number: &str, unit: &str) -> u64 {
    let scale: f64 = match unit {
        "KiB" => 1024.0,
        "MiB" => 1024.0 * 1024.0,
        "GiB" => 1024.0 * 1024.0 * 1024.0,
        "TiB" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => 1.0,
    };
    (number.parse::<f64>().unwrap_or(0.0) * scale) as u64
}

fn decode_html(text: &str) -> String {
    text.replace("&#39;", "'").replace("&quot;", "\"").replace("&lt;", "<").replace("&gt;", ">").replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESULTS: &str = r#"<tbody>
    <tr class="success">
        <td colspan="2">
            <a href="/view/2163848" title="[One Pace][153-155] Drum Island 08 [1080p][FA5C67B1].mkv">x</a>
        </td>
        <td class="text-center">
            <a href="/download/2163848.torrent"></a>
            <a href="magnet:?xt=urn:btih:1854D3A4591289A1648C9775800F296812E4E480&amp;dn=x&amp;tr=http%3A%2F%2Fnyaa.tracker.wf%3A7777%2Fannounce"></a>
        </td>
        <td class="text-center">1015.1 MiB</td>
        <td class="text-center" data-timestamp="1789868818">2026-09-20 01:46</td>
        <td class="text-center">74</td>
        <td class="text-center">2</td>
        <td class="text-center">460</td>
    </tr>
    <tr class="default"><td>broken row</td></tr>
    </tbody>"#;

    #[test]
    fn reads_nyaa_result_rows() {
        let rows = NyaaRows::new().parse(RESULTS);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.info_hash, "1854d3a4591289a1648c9775800f296812e4e480");
        assert_eq!((row.nyaa_id, row.seeders, row.leechers, row.published_at), (2163848, 74, 2, 1789868818));
        assert_eq!(row.size_bytes, 1_064_409_497);
        assert!(row.magnet.contains("&dn=x&tr="));
    }

    #[test]
    fn reads_a_nested_file_list() {
        let page = r#"<div class="torrent-file-list panel-body"><ul>
            <li><a href="" class="folder"><i class="fa fa-folder-open"></i>[One Pace][909-924] Wano Act 1</a><ul>
                <li><i class="fa fa-file"></i>[One Pace] Wano 01 [F15AFDE0].mkv <span class="file-size">(866.7 MiB)</span></li>
                <li><a href="" class="folder">Extras</a><ul>
                    <li><i class="fa fa-file"></i>Buggy&#39;s poster.png <span class="file-size">(1.0 KiB)</span></li>
                </ul></li>
                <li><i class="fa fa-file"></i>[One Pace] Wano 02 [8538EDC4].mkv <span class="file-size">(1.5 GiB)</span></li>
            </ul></li></ul></div>"#;
        let paths: Vec<_> = file_list(page).into_iter().map(|f| f.path).collect();
        assert_eq!(
            paths,
            [
                "[One Pace][909-924] Wano Act 1/[One Pace] Wano 01 [F15AFDE0].mkv",
                "[One Pace][909-924] Wano Act 1/Extras/Buggy's poster.png",
                "[One Pace][909-924] Wano Act 1/[One Pace] Wano 02 [8538EDC4].mkv",
            ]
        );
    }

    #[test]
    fn reads_sheet_tabs() {
        let page = r#"items.push({name: "Arc Overview", pageUrl: "https://x/htmlview/sheet?headers\x3dfalse\x26gid=0", gid: "0"});
            items.push({name: "The Adventures of Buggy\x27s Crew", pageUrl: "https://x?gid=1955340365"});"#;
        assert_eq!(
            guide_tabs(page),
            [
                ("Arc Overview".to_string(), "0".to_string()),
                ("The Adventures of Buggy's Crew".to_string(), "1955340365".to_string())
            ]
        );
    }

    #[test]
    fn single_file_titles_end_with_a_crc32() {
        assert!(is_single_file("[One Pace][1000] Wano 55 [1080p][2501AC5A].mkv"));
        assert!(!is_single_file("[One Pace][909-924] Wano Act 1"));
        assert!(!is_single_file("[One Pace][23-41] Syrup Village [1080p]"));
    }
}
