//! Small helpers shared by the semantic passes.

/// Edit distance counting insertions, deletions, substitutions and transpositions of
/// adjacent characters (optimal string alignment), for "did you mean ...?" suggestions.
pub fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().map(|c| c.to_ascii_lowercase()).collect();
    let b: Vec<char> = b.chars().map(|c| c.to_ascii_lowercase()).collect();
    let (n, m) = (a.len(), b.len());
    let mut d = vec![vec![0usize; m + 1]; n + 1];
    for (i, row) in d.iter_mut().enumerate() {
        row[0] = i;
    }
    for j in 0..=m {
        d[0][j] = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            d[i][j] = (d[i - 1][j] + 1).min(d[i][j - 1] + 1).min(d[i - 1][j - 1] + cost);
            if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                d[i][j] = d[i][j].min(d[i - 2][j - 2] + 1);
            }
        }
    }
    d[n][m]
}

/// The candidate closest to `name`, if it is close enough to be a plausible typo.
/// Deterministic: ties prefer the longer candidate (a forgotten letter is a more common
/// typo than an extra one), then alphabetical order.
pub fn best_match<'a>(name: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<&'a str> {
    let max = (name.chars().count() / 3).max(1);
    let mut best: Option<(&str, usize)> = None;
    for c in candidates {
        if c == name {
            continue;
        }
        let d = edit_distance(name, c);
        if d > max {
            continue;
        }
        let better = match best {
            None => true,
            Some((b, bd)) => d < bd || (d == bd && (c.len() > b.len() || (c.len() == b.len() && c < b))),
        };
        if better {
            best = Some((c, d));
        }
    }
    best.map(|(c, _)| c)
}

/// Names of primitive types, which cannot be redefined.
pub const PRIMITIVE_TYPES: &[&str] =
    &["i8", "i16", "i32", "i64", "isize", "u8", "u16", "u32", "u64", "usize", "f32", "f64", "bool", "char", "Never", "Ptr"];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions() {
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("nrom", "norm"), 1);
        assert_eq!(best_match("prinln", ["println", "print", "panic"]), Some("println"));
        assert_eq!(best_match("printn", ["print", "println"]), Some("println"));
        assert_eq!(best_match("printn", ["println", "print"]), Some("println"));
        assert_eq!(best_match("xyz", ["println"]), None);
    }
}
