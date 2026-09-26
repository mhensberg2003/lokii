//! Text clean-up for AniList's HTML-flavoured `description` and `streamingEpisodes` titles.

/// Cleans an AniList `description` field: converts `<br>` to newlines, strips remaining
/// HTML tags, decodes basic HTML entities, and drops a trailing "(Source: ...)" note.
pub fn clean_description(raw: &str) -> String {
    let with_breaks = replace_breaks(raw);
    let without_tags = strip_tags(&with_breaks);
    let decoded = decode_entities(&without_tags);
    drop_trailing_source_note(&decoded).trim().to_string()
}

/// The `<br>` spellings we fold into a newline, longest first so a match is unambiguous.
const BR_VARIANTS: [&str; 3] = ["<br />", "<br/>", "<br>"];

/// Replaces `<br>`, `<br/>` and `<br />` (any case) with a newline.
fn replace_breaks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        let rest = &input[i..];
        let matched = BR_VARIANTS
            .iter()
            .find(|variant| rest.get(..variant.len()).is_some_and(|p| p.eq_ignore_ascii_case(variant)));
        if let Some(variant) = matched {
            out.push('\n');
            i += variant.len();
        } else {
            let ch = rest.chars().next().expect("i < input.len()");
            out.push(ch);
            i += ch.len_utf8();
        }
    }
    out
}

/// Removes every `<...>` tag, keeping the text between them.
fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    for ch in input.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    out
}

/// Decodes the handful of HTML entities AniList descriptions actually use.
fn decode_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();
    while let Some((i, ch)) = chars.next() {
        if ch != '&' {
            out.push(ch);
            continue;
        }
        let Some(end) = input[i..].find(';').map(|p| i + p) else {
            out.push(ch);
            continue;
        };
        // Entities are short; refuse to treat a distant ';' as part of one.
        if end - i > 12 {
            out.push(ch);
            continue;
        }
        let entity = &input[i + 1..end];
        if let Some(decoded) = decode_one_entity(entity) {
            out.push(decoded);
            // Skip the chars we just consumed (up to and including ';').
            while let Some(&(next_i, _)) = chars.peek() {
                if next_i > end {
                    break;
                }
                chars.next();
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn decode_one_entity(entity: &str) -> Option<char> {
    match entity {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" | "#39" | "#x27" => Some('\''),
        "nbsp" => Some(' '),
        _ => {
            if let Some(hex) = entity.strip_prefix("#x").or_else(|| entity.strip_prefix("#X")) {
                u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
            } else if let Some(dec) = entity.strip_prefix('#') {
                dec.parse::<u32>().ok().and_then(char::from_u32)
            } else {
                None
            }
        }
    }
}

/// Drops a trailing "(Source: ...)" note (AniList's convention for attributing a summary),
/// including the newline(s) that separate it from the rest of the description.
fn drop_trailing_source_note(input: &str) -> &str {
    let lower = input.to_lowercase();
    let Some(pos) = lower.rfind("(source:").or_else(|| lower.rfind("[source:")) else {
        return input;
    };
    // Only drop it if it is the last non-blank content (a trailing note, not prose).
    let before = &input[..pos];
    let after = input[pos..].trim();
    let looks_like_trailing_note = after.ends_with(')') || after.ends_with(']');
    if looks_like_trailing_note {
        before
    } else {
        input
    }
}

/// Parses a `streamingEpisodes[].title` string shaped like `"Episode 12 - The Title"`.
/// Returns `None` when the title does not match that shape.
pub fn parse_streaming_title(title: &str) -> Option<(u32, String)> {
    let rest = title.strip_prefix("Episode ")?;
    let (number, name) = rest.split_once(" - ")?;
    let number: u32 = number.trim().parse().ok()?;
    let name = name.trim();
    if name.is_empty() {
        return None;
    }
    Some((number, name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_and_converts_breaks() {
        let raw = "<i>Line one</i><br>Line two<br/>Line three<br />end";
        assert_eq!(clean_description(raw), "Line one\nLine two\nLine three\nend");
    }

    #[test]
    fn decodes_common_entities() {
        let raw = "Rock &amp; Roll &lt;tag&gt; &quot;quote&quot; &#39;s&#39; caf&#233;";
        assert_eq!(clean_description(raw), "Rock & Roll <tag> \"quote\" 's' café");
    }

    #[test]
    fn drops_trailing_source_note() {
        let raw = "A great show about friendship.<br><br>(Source: AniList)";
        assert_eq!(clean_description(raw), "A great show about friendship.");
    }

    #[test]
    fn drops_trailing_source_note_case_insensitive_and_bracketed() {
        let raw = "Plot summary here.\n[Source: MAL]";
        assert_eq!(clean_description(raw), "Plot summary here.");
    }

    #[test]
    fn keeps_source_mentioned_mid_sentence() {
        let raw = "The source of the conflict (Source of all evil) is revealed later.";
        assert_eq!(clean_description(raw), raw);
    }

    #[test]
    fn parses_episode_title() {
        assert_eq!(
            parse_streaming_title("Episode 12 - The Reunion"),
            Some((12, "The Reunion".to_string()))
        );
    }

    #[test]
    fn rejects_titles_that_do_not_match() {
        assert_eq!(parse_streaming_title("The Reunion"), None);
        assert_eq!(parse_streaming_title("Episode - The Reunion"), None);
        assert_eq!(parse_streaming_title("Episode 12"), None);
        assert_eq!(parse_streaming_title("Episode 12 - "), None);
    }
}
