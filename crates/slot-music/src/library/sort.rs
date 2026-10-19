//! How names are put in order, as players have them: a leading "The" does not count, so The
//! Beatles are under B; names that begin with a digit or a symbol come first, together, under
//! "#"; and numbers in names count as numbers, so Track 2 comes before Track 10.

use crate::text;

/// The key a name sorts by.
pub fn sort_key(name: &str) -> (bool, Vec<Part>) {
    let bare = bare(name);
    (starts_with_letter(bare), parts(&bare.to_uppercase()))
}

/// A run of a name: a number, which sorts by its value, before the rest, which sort as text.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Part {
    Number(u128),
    Text(String),
}

/// `name` in runs of digits and of the rest.
fn parts(name: &str) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut rest = name;
    while let Some(c) = rest.chars().next() {
        let digit = c.is_ascii_digit();
        let end = rest
            .find(|c: char| c.is_ascii_digit() != digit)
            .unwrap_or(rest.len());
        let (run, after) = rest.split_at(end);
        parts.push(if digit {
            run.parse()
                .map_or_else(|_| Part::Text(run.into()), Part::Number)
        } else {
            Part::Text(run.into())
        });
        rest = after;
    }
    parts
}

/// The letter a name is listed under, or "#".
pub fn initial(name: &str) -> char {
    let bare = bare(name);
    if starts_with_letter(bare) {
        bare.chars()
            .next()
            .and_then(|c| c.to_uppercase().next())
            .unwrap_or('#')
    } else {
        '#'
    }
}

/// The name without what goes before it and does not count: spaces, and an article the
/// language has (`text::ARTICLES`), unless that is all there is.
fn bare(name: &str) -> &str {
    let name = name.trim_start();
    let after = |article: &str| {
        let rest = name.get(article.len()..)?;
        let head = name.get(..article.len())?;
        let word = head.eq_ignore_ascii_case(article) && rest.starts_with(' ');
        (word && !rest.trim().is_empty()).then(|| rest.trim_start())
    };
    text::ARTICLES.iter().find_map(|a| after(a)).unwrap_or(name)
}

fn starts_with_letter(name: &str) -> bool {
    name.chars().next().is_some_and(char::is_alphabetic)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_does_not_count_and_digits_and_symbols_come_first_together() {
        let mut names = [
            "Zebra",
            "the Beatles",
            "(What's the Story)",
            "Abba",
            "1999",
            "The",
            "Theatre",
            "#1 Record",
        ];
        names.sort_by_cached_key(|n| sort_key(n));
        assert_eq!(
            names,
            [
                "1999",
                "#1 Record",
                "(What's the Story)",
                "Abba",
                "the Beatles",
                "The",
                "Theatre",
                "Zebra",
            ]
        );
        let mut tracks = [
            "Track 10", "Track 2", "Track 1", "track 2b", "Vol. 9", "Vol. 10",
        ];
        tracks.sort_by_cached_key(|n| sort_key(n));
        assert_eq!(
            tracks,
            ["Track 1", "Track 2", "track 2b", "Track 10", "Vol. 9", "Vol. 10"],
            "numbers as numbers"
        );
        assert_eq!(initial("The Beatles"), 'B');
        assert_eq!(initial("(What's the Story)"), '#');
        assert_eq!(initial("1999"), '#');
        assert_eq!(initial("élan"), 'É');
        assert_eq!(initial("The"), 'T', "a band called The");
    }
}
