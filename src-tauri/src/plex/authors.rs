//! Parse multi-author / collab credit strings from Plex metadata.

/// Split a credit string into individual author names.
/// Prefers `&` / ` and ` / ` with ` / `/` before commas (avoids "Smith, Jr.").
pub fn split_authors(raw: &str) -> Vec<String> {
    let s = raw.trim();
    if s.is_empty() {
        return vec![];
    }

    let lower = s.to_ascii_lowercase();
    let has_strong_sep = lower.contains(" & ")
        || lower.contains(" and ")
        || lower.contains(" with ")
        || s.contains('/')
        || s.contains(';');

    let parts: Vec<String> = if has_strong_sep {
        // Normalize strong separators to |
        let mut buf = String::new();
        let mut i = 0;
        let chars: Vec<char> = s.chars().collect();
        while i < chars.len() {
            let rest: String = chars[i..].iter().collect();
            let rest_l = rest.to_ascii_lowercase();
            if rest_l.starts_with(" and ") {
                buf.push('|');
                i += 5;
            } else if rest_l.starts_with(" with ") {
                buf.push('|');
                i += 6;
            } else if rest_l.starts_with(" & ") {
                buf.push('|');
                i += 3;
            } else if chars[i] == '&' || chars[i] == '/' || chars[i] == ';' {
                buf.push('|');
                i += 1;
            } else {
                buf.push(chars[i]);
                i += 1;
            }
        }
        buf.split('|')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(|p| p.to_string())
            .collect()
    } else if s.contains(',') {
        // Only split on comma if it looks like "A, B" (two+ name-like parts)
        let comma_parts: Vec<&str> = s.split(',').map(str::trim).filter(|p| !p.is_empty()).collect();
        if comma_parts.len() >= 2
            && comma_parts.iter().all(|p| {
                // Reject "Last, Jr." style single-author — last part very short suffix
                let words = p.split_whitespace().count();
                words >= 1 && !(p.len() <= 4 && words == 1 && p.chars().all(|c| c.is_alphabetic()))
            })
            && !looks_like_last_first(s)
        {
            comma_parts.into_iter().map(|p| p.to_string()).collect()
        } else {
            vec![s.to_string()]
        }
    } else {
        vec![s.to_string()]
    };

    dedupe_preserve(parts)
}

fn looks_like_last_first(s: &str) -> bool {
    // "Weber, David" — one comma, second part is single token first name
    let parts: Vec<&str> = s.split(',').map(str::trim).collect();
    parts.len() == 2 && parts[1].split_whitespace().count() == 1 && parts[1].len() < 20
}

fn dedupe_preserve(parts: Vec<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for p in parts {
        let key = p.to_ascii_lowercase();
        if seen.insert(key) {
            out.push(p);
        }
    }
    out
}

/// Build authors list + display string from Plex album metadata fields.
pub fn authors_from_metadata(parent_title: Option<&str>, original_title: Option<&str>) -> (Vec<String>, Option<String>) {
    let mut authors = Vec::new();
    if let Some(p) = parent_title.map(str::trim).filter(|s| !s.is_empty()) {
        authors.extend(split_authors(p));
    }
    if authors.is_empty() {
        if let Some(o) = original_title.map(str::trim).filter(|s| !s.is_empty()) {
            authors.extend(split_authors(o));
        }
    }
    let authors = dedupe_preserve(authors);
    let display = if authors.is_empty() {
        None
    } else if authors.len() == 1 {
        Some(authors[0].clone())
    } else if authors.len() == 2 {
        Some(format!("{} & {}", authors[0], authors[1]))
    } else {
        Some(authors.join(", "))
    };
    (authors, display)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_ampersand() {
        let a = split_authors("David Weber & Timothy Zahn");
        assert_eq!(a, vec!["David Weber", "Timothy Zahn"]);
    }

    #[test]
    fn split_and() {
        let a = split_authors("David Weber and Timothy Zahn");
        assert_eq!(a, vec!["David Weber", "Timothy Zahn"]);
    }
}
