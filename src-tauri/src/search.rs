use crate::models::{ParamType, ShortCutItem};
use regex::Regex;

/// Filter and rank shortcut items by keyword
pub fn filter_items(
    items: &[ShortCutItem],
    keyword: &str,
    enable_regex: bool,
    match_anywhere: bool,
    max_results: usize,
) -> Vec<ShortCutItem> {
    if keyword.is_empty() {
        // Return all items sorted by frequency
        let mut sorted: Vec<ShortCutItem> = items.to_vec();
        sorted.sort_by(|a, b| b.freq.cmp(&a.freq).then(a.shortcut.cmp(&b.shortcut)));
        sorted.truncate(max_results);
        return sorted;
    }

    let kw_lower = keyword.to_lowercase();

    // Preprocess keyword for regex mode
    let regex_pattern = if enable_regex {
        let mut pat = kw_lower.clone();
        // Replace standalone * with .* and ? with .
        pat = pat.replace(".*", "\x00");
        pat = pat.replace('*', ".*");
        pat = pat.replace('\x00', ".*");
        pat = pat.replace('?', ".");
        Some(Regex::new(&pat).ok())
    } else {
        None
    };

    let mut results: Vec<ShortCutItem> = Vec::new();

    for item in items {
        let sc_lower = item.shortcut.to_lowercase();

        let match_pos: Option<usize> = if enable_regex {
            if let Some(Some(ref re)) = regex_pattern {
                re.find(&sc_lower).map(|m| m.start() + 1)
            } else {
                // Regex failed to compile, fall back to simple match
                sc_lower.find(&kw_lower).map(|p| p + 1)
            }
        } else {
            sc_lower.find(&kw_lower).map(|p| p + 1)
        };

        if let Some(pos) = match_pos {
            // If not match_anywhere, must match from start
            if !match_anywhere && pos > 1 {
                continue;
            }

            let mut ranked = item.clone();
            // Ranking formula (from original ALTRun):
            // rank = 1024 + freq*4 - match_pos*128 - (len_diff)*16
            let len_diff = (item.shortcut.len() as i32) - (keyword.len() as i32);
            ranked.rank = 1024 + item.freq * 4 - (pos as i32) * 128 - len_diff * 16;

            results.push(ranked);
        }
    }

    // Sort by rank descending
    results.sort_by(|a, b| b.rank.cmp(&a.rank));
    results.truncate(max_results);
    results
}
