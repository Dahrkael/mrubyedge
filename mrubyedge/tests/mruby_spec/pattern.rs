// File.fnmatch-style assertion patterns, including braces and dot/slash matches.
// Operate on bytes, as mruby's test driver does.
pub fn matches(pattern: &str, value: &str) -> bool {
    match_bytes(pattern.as_bytes(), value.as_bytes())
}

fn match_bytes(p: &[u8], s: &[u8]) -> bool {
    let Some((&first, tail)) = p.split_first() else {
        return s.is_empty();
    };
    match first {
        b'*' => (0..=s.len()).any(|i| match_bytes(tail, &s[i..])),
        b'?' => !s.is_empty() && match_bytes(tail, &s[1..]),
        b'\\' => {
            tail.first() == s.first()
                && !tail.is_empty()
                && !s.is_empty()
                && match_bytes(&tail[1..], &s[1..])
        }
        b'[' => {
            if s.is_empty() {
                return false;
            }
            let mut i = 1;
            let negated = matches!(p.get(i), Some(b'!' | b'^'));
            if negated {
                i += 1;
            }
            let mut found = false;
            while i < p.len() && p[i] != b']' {
                if p[i] == b'\\' {
                    i += 1;
                }
                if i >= p.len() {
                    return false;
                }
                let low = p[i];
                i += 1;
                if p.get(i) == Some(&b'-') && p.get(i + 1).is_some_and(|c| *c != b']') {
                    i += 1;
                    if p[i] == b'\\' {
                        i += 1;
                    }
                    if i >= p.len() {
                        return false;
                    }
                    found |= low <= s[0] && s[0] <= p[i];
                    i += 1;
                } else {
                    found |= low == s[0];
                }
            }
            i < p.len() && found != negated && match_bytes(&p[i + 1..], &s[1..])
        }
        b'{' => {
            let mut depth = 0;
            let mut start = 1;
            let mut alternatives = Vec::new();
            let mut i = 1;
            while i < p.len() {
                match p[i] {
                    b'\\' => i += 1,
                    b'{' => depth += 1,
                    b'}' if depth == 0 => {
                        alternatives.push(&p[start..i]);
                        return alternatives.into_iter().any(|part| {
                            let mut expanded = part.to_vec();
                            expanded.extend_from_slice(&p[i + 1..]);
                            match_bytes(&expanded, s)
                        });
                    }
                    b'}' => depth -= 1,
                    b',' if depth == 0 => {
                        alternatives.push(&p[start..i]);
                        start = i + 1;
                    }
                    _ => {}
                }
                i += 1;
            }
            false
        }
        _ => s.first() == Some(&first) && match_bytes(tail, &s[1..]),
    }
}

#[test]
fn assertion_patterns() {
    for (p, s, expected) in [
        ("*", ".path/to/file", true),
        ("a?c", "abc", true),
        ("a?c", "ac", false),
        ("[a-c]", "b", true),
        ("[^a-c]", "b", false),
        ("[!a-c]", "z", true),
        (r"a\*b", "a*b", true),
        ("{ab,{cd,ef}}?", "efg", true),
        ("{ab,cd}", "abx", false),
        ("[", "x", false),
        ("", "", true),
        ("?", "", false),
    ] {
        assert_eq!(matches(p, s), expected, "{p:?}, {s:?}");
    }
}
