//! Checks a mod's declared Minecraft range (from `fabric.mod.json`) against
//! the instance version. Covers the predicate forms Fabric mods actually use:
//! `*`, `26.3`, `26.3.x`, `>=26.3`, `<26.4`, `~26.3`, `^26.3`, space-separated
//! AND (`>=26.3- <26.4`), and JSON arrays as OR.

use std::cmp::Ordering;

use serde_json::Value;

/// `Some(true)` compatible, `Some(false)` incompatible, `None` if we can't tell.
pub fn satisfies(predicate: &Value, version: &str) -> Option<bool> {
    match predicate {
        Value::String(s) => satisfies_str(s, version),
        Value::Array(items) => {
            let mut any_known = false;
            for item in items {
                match satisfies(item, version) {
                    Some(true) => return Some(true),
                    Some(false) => any_known = true,
                    None => {}
                }
            }
            any_known.then_some(false)
        }
        _ => None,
    }
}

/// Human-readable form of the predicate for the UI.
pub fn describe(predicate: &Value) -> Option<String> {
    match predicate {
        Value::String(s) => Some(s.clone()),
        Value::Array(items) => {
            let parts: Vec<String> = items.iter().filter_map(describe).collect();
            (!parts.is_empty()).then(|| parts.join(" or "))
        }
        _ => None,
    }
}

fn satisfies_str(predicate: &str, version: &str) -> Option<bool> {
    let terms: Vec<&str> = predicate.split_whitespace().collect();
    if terms.is_empty() {
        return None;
    }
    for term in terms {
        if !term_matches(term, version)? {
            return Some(false);
        }
    }
    Some(true)
}

fn term_matches(term: &str, version: &str) -> Option<bool> {
    if term == "*" {
        return Some(true);
    }
    let (op, rest) = split_op(term);
    // Fabric uses a trailing `-` to mean "the earliest pre-release of this
    // version", so `>=26.3-` also accepts 26.3 release candidates.
    let include_pre = rest.ends_with('-');
    let rest = rest.trim_end_matches('-');

    if op.is_empty() && (rest.ends_with(".x") || rest.ends_with(".*") || rest.ends_with(".X")) {
        let prefix = &rest[..rest.len() - 2];
        let want = parse(prefix)?;
        let have = parse(version)?;
        return Some(have.core.len() >= want.core.len() && have.core[..want.core.len()] == want.core[..]);
    }

    let mut want = parse(rest)?;
    if include_pre && want.pre.is_none() {
        want.pre = Some(String::new()); // sorts below every real pre-release tag
    }
    let have = parse(version)?;
    let ord = have.cmp(&want);
    Some(match op {
        "" | "=" => ord == Ordering::Equal,
        ">=" => ord != Ordering::Less,
        ">" => ord == Ordering::Greater,
        "<=" => ord != Ordering::Greater,
        "<" => ord == Ordering::Less,
        "~" => ord != Ordering::Less && have.core_cmp(&want.bump(1)) == Ordering::Less,
        "^" => ord != Ordering::Less && have.core_cmp(&want.bump(0)) == Ordering::Less,
        _ => return None,
    })
}

fn split_op(term: &str) -> (&str, &str) {
    for op in [">=", "<=", ">", "<", "=", "~", "^"] {
        if let Some(rest) = term.strip_prefix(op) {
            return (op, rest);
        }
    }
    ("", term)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Ver {
    core: Vec<u64>,
    pre: Option<String>,
}

impl Ver {
    fn core_cmp(&self, other: &Ver) -> Ordering {
        let len = self.core.len().max(other.core.len());
        for i in 0..len {
            let a = self.core.get(i).copied().unwrap_or(0);
            let b = other.core.get(i).copied().unwrap_or(0);
            match a.cmp(&b) {
                Ordering::Equal => continue,
                o => return o,
            }
        }
        Ordering::Equal
    }

    /// Next version at `index` (0 = major, 1 = minor), as a release.
    fn bump(&self, index: usize) -> Ver {
        let mut core: Vec<u64> = self.core.iter().take(index + 1).copied().collect();
        while core.len() <= index {
            core.push(0);
        }
        core[index] += 1;
        Ver { core, pre: None }
    }
}

impl Ord for Ver {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.core_cmp(other) {
            Ordering::Equal => match (&self.pre, &other.pre) {
                (None, None) => Ordering::Equal,
                (None, Some(_)) => Ordering::Greater, // release > pre-release
                (Some(_), None) => Ordering::Less,
                (Some(a), Some(b)) => a.cmp(b),
            },
            o => o,
        }
    }
}

impl PartialOrd for Ver {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

fn parse(s: &str) -> Option<Ver> {
    let s = s.trim();
    let (core, pre) = match s.split_once('-') {
        Some((c, p)) if !p.is_empty() => (c, Some(p.to_string())),
        Some((c, _)) => (c, None),
        None => (s, None),
    };
    let core = core.split_once('+').map(|(c, _)| c).unwrap_or(core);
    let nums: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    let nums = nums?;
    if nums.is_empty() {
        return None;
    }
    Some(Ver { core: nums, pre })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ok(p: Value) -> Option<bool> {
        satisfies(&p, "26.3")
    }

    #[test]
    fn common_forms() {
        assert_eq!(ok(json!("*")), Some(true));
        assert_eq!(ok(json!("26.3")), Some(true));
        assert_eq!(ok(json!("26.2")), Some(false));
        assert_eq!(ok(json!(">=26.3")), Some(true));
        assert_eq!(ok(json!(">=26.3-")), Some(true));
        assert_eq!(ok(json!(">=26.3- <26.4")), Some(true));
        assert_eq!(ok(json!(">=26.1 <26.2")), Some(false));
        assert_eq!(ok(json!("~26.3")), Some(true));
        assert_eq!(ok(json!("~26.2")), Some(false));
        assert_eq!(ok(json!("^26.1")), Some(true));
        assert_eq!(ok(json!("26.x")), Some(true));
        assert_eq!(ok(json!("26.3.x")), Some(true));
        assert_eq!(ok(json!("1.21.x")), Some(false));
        assert_eq!(ok(json!(["26.2", "26.3"])), Some(true));
        assert_eq!(ok(json!(["1.21.11", "26.1"])), Some(false));
        assert_eq!(ok(json!("<26.3")), Some(false));
    }

    #[test]
    fn prereleases_sort_before_release() {
        assert_eq!(satisfies(&json!(">=26.3"), "26.3-rc.1"), Some(false));
        assert_eq!(satisfies(&json!(">=26.3-"), "26.3"), Some(true));
        assert_eq!(satisfies(&json!(">=26.3-"), "26.3-rc.1"), Some(true));
        // Semver rules: `<26.4` still admits 26.4 pre-releases; `<26.4-` doesn't.
        assert_eq!(satisfies(&json!(">=26.3- <26.4"), "26.4-alpha.1"), Some(true));
        assert_eq!(satisfies(&json!(">=26.3- <26.4-"), "26.4-alpha.1"), Some(false));
    }

    #[test]
    fn garbage_is_unknown() {
        assert_eq!(ok(json!("banana")), None);
        assert_eq!(ok(json!(42)), None);
    }
}
