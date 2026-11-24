//! `Co-authored-by:` trailers, parsed client-side so it works against plain Forgejo too.
//! Same rules as Damast's server (modules/damast/coauthor): last paragraph only, git-style
//! "Name <email>", deduplicated by email, without the author.

const KEY: &str = "co-authored-by:";

/// At most this many co-authors per commit, like the server.
const MAX: usize = 100;

/// Control and bidi override characters (Go's unicode.IsControl / unicode.Bidi_Control): they could
/// reorder the surrounding text or inject terminal escapes.
fn unsafe_char(c: char) -> bool {
    c.is_control()
        || matches!(c, '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

#[derive(Debug, PartialEq, Eq)]
pub struct CoAuthor {
    pub name: String,
    pub email: String,
}

/// Co-authors from the trailer block (last paragraph) of a commit message.
pub fn parse(message: &str, author_email: &str) -> Vec<CoAuthor> {
    let message = message.replace("\r\n", "\n");
    let message = message.trim();
    let Some(i) = message.rfind("\n\n") else {
        return Vec::new(); // the first paragraph is the subject, never a trailer block (git interpret-trailers)
    };
    let block = &message[i + 2..];

    let mut seen = std::collections::HashSet::from([author_email.trim().to_lowercase()]);
    let mut out = Vec::new();
    for line in block.lines().map(str::trim) {
        let Some(value) = line
            .get(..KEY.len())
            .filter(|k| k.eq_ignore_ascii_case(KEY))
            .map(|_| line[KEY.len()..].trim())
        else {
            continue;
        };
        let (Some(lt), true) = (value.rfind('<'), value.ends_with('>')) else {
            continue;
        };
        let email = value[lt + 1..value.len() - 1].trim();
        let valid = matches!(email.find('@'), Some(at) if at > 0 && at + 1 < email.len() && email.rfind('@') == Some(at))
            && !email.contains(['<', '>', ' '])
            && !email.contains(unsafe_char);
        if !valid || !seen.insert(email.to_lowercase()) {
            continue;
        }
        let name: String = value[..lt].chars().filter(|&c| !unsafe_char(c)).collect();
        let name = name.trim().trim_matches('"');
        out.push(CoAuthor {
            name: if name.is_empty() { email } else { name }.to_string(),
            email: email.to_string(),
        });
        if out.len() == MAX {
            break;
        }
    }
    out
}

/// "Author and Name" for one co-author, "Author and N others" for more, like on Damast's web UI.
pub fn summary(author: &str, co: &[CoAuthor]) -> String {
    match co {
        [] => author.to_string(),
        [one] => format!("{author} and {}", one.name),
        more => format!("{author} and {} others", more.len()),
    }
}

/// " (Author and …)" for `--oneline` output, empty when the commit has no co-authors.
pub fn oneline_suffix(message: &str, author: Option<&forgejo_api::structs::CommitUser>) -> String {
    let name = author.and_then(|a| a.name.as_deref()).unwrap_or("?");
    let email = author.and_then(|a| a.email.as_deref()).unwrap_or("");
    let co = parse(message, email);
    if co.is_empty() {
        String::new()
    } else {
        format!(" ({})", summary(name, &co))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn co(name: &str, email: &str) -> CoAuthor {
        CoAuthor {
            name: name.into(),
            email: email.into(),
        }
    }

    #[test]
    fn parses_trailer_block() {
        let msg = "feat: x\r\n\r\nbody\r\n\r\nCo-authored-by: Bob <bob@x.org>\r\nco-AUTHORED-by: dependabot[bot] <1+dependabot[bot]@users.noreply.github.com>\r\nCo-authored-by: bob <BOB@x.org>\r\nCo-authored-by: Me <A@x.org>\r\nCo-authored-by: \"Doe, John\" <j@x.org>\r\nSigned-off-by: A <a@x.org>";
        assert_eq!(
            parse(msg, "a@x.org"),
            vec![
                co("Bob", "bob@x.org"),
                co(
                    "dependabot[bot]",
                    "1+dependabot[bot]@users.noreply.github.com"
                ),
                co("Doe, John", "j@x.org"),
            ]
        );
    }

    #[test]
    fn skips_what_is_not_a_trailer() {
        assert!(parse("Co-authored-by: Bob <bob@x.org>", "").is_empty());
        assert!(parse("feat\n\nCo-authored-by: Bob <bob@x.org>\n\nmore text", "").is_empty());
        assert!(parse("feat\n\nCo-authored-by: A <a>\nCo-authored-by: B <@x>\nCo-authored-by: C <c@x@y>\nCo-authored-by: no email\nCo-authored-by:", "").is_empty());
        assert_eq!(
            parse("feat\n\nCo-authored-by: <only@x.org>", ""),
            vec![co("only@x.org", "only@x.org")]
        );
    }

    /// Same vectors as Damast's server parser (modules/damast/coauthor/testdata/parse_vectors.json): keep both copies equal.
    #[test]
    fn matches_server_vectors() {
        #[derive(serde::Deserialize)]
        struct Case {
            name: String,
            message: String,
            author: String,
            want: Vec<(String, String)>,
        }
        let cases: Vec<Case> =
            serde_json::from_str(include_str!("coauthors_vectors.json")).unwrap();
        for c in cases {
            let want: Vec<CoAuthor> = c.want.iter().map(|(n, e)| co(n, e)).collect();
            assert_eq!(parse(&c.message, &c.author), want, "{}", c.name);
        }
    }

    #[test]
    fn caps_huge_trailer_blocks() {
        let msg = (0..100_000).fold("mob\n\n".to_string(), |m, i| {
            m + &format!("Co-authored-by: N{i} <n{i}@x.org>\n")
        });
        let got = parse(&msg, "");
        assert_eq!(got.len(), MAX);
        assert_eq!(got[99], co("N99", "n99@x.org"));
    }

    #[test]
    fn summarises() {
        assert_eq!(summary("Jonas", &[]), "Jonas");
        assert_eq!(summary("Jonas", &[co("Mira", "m@x")]), "Jonas and Mira");
        assert_eq!(
            summary("Jonas", &[co("Mira", "m@x"), co("Lea", "l@x")]),
            "Jonas and 2 others"
        );
    }
}
