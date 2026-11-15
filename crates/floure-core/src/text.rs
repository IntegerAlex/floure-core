//! Deterministic text repair for dictation output.
//!
//! This layer does not generate words. It only repairs the mechanical
//! artifacts every ASR engine leaves behind: premature sentence breaks,
//! missing question marks, punctuation spacing, sentence-start casing, and a
//! missing terminal full stop. The optional LLM "Polish" pass sits after this.

use std::sync::OnceLock;

use regex::Regex;

/// Words that can open a sentence we are willing to re-capitalise. Keeping a
/// list avoids turning `iPhone` into `IPhone` when a transcript starts with a
/// proper noun.
const STARTERS: &[&str] = &[
    "a",
    "an",
    "the",
    "this",
    "that",
    "these",
    "those",
    "it",
    "we",
    "you",
    "they",
    "he",
    "she",
    "i",
    "and",
    "but",
    "so",
    "if",
    "when",
    "what",
    "how",
    "why",
    "where",
    "who",
    "which",
    "can",
    "could",
    "do",
    "does",
    "did",
    "is",
    "are",
    "was",
    "were",
    "will",
    "would",
    "should",
    "let",
    "please",
    "here",
    "there",
    "my",
    "our",
    "your",
    "their",
    "his",
    "her",
    "its",
    "all",
    "no",
    "yes",
    "okay",
    "ok",
    "thanks",
    "thank",
    "hello",
    "hi",
    "hey",
    "good",
    "great",
    "sorry",
    "note",
    "first",
    "second",
    "next",
    "then",
    "now",
    "today",
    "tomorrow",
    "yesterday",
    "use",
    "add",
    "remove",
    "fix",
    "check",
    "open",
    "close",
    "start",
    "stop",
    "make",
    "create",
    "update",
    "delete",
    "send",
    "call",
    "tell",
    "show",
    "give",
    "take",
    "get",
    "put",
    "keep",
    "run",
    "try",
    "go",
    "come",
    "look",
    "read",
    "write",
    "say",
    "ask",
    "help",
    "need",
    "want",
    "like",
    "know",
    "think",
    "feel",
    "see",
    "find",
];

/// A word after which a period is likely premature rather than a real
/// sentence end. Kept small: the cost of a wrong join is worse than leaving a
/// fragment.
const JOIN_AFTER: &[&str] = &[
    "the", "a", "an", "of", "to", "in", "on", "at", "for", "with", "and", "or", "but", "that",
    "this", "these", "those", "my", "our", "your", "their", "his", "her", "its", "is", "are",
    "was", "were", "be", "been", "being", "can", "could", "will", "would", "should", "do", "does",
    "did", "have", "has", "had", "not",
];

/// A word that can continue a sentence. A lowercase word here after a
/// terminator is the signal that the engine punctuated a pause, not a
/// sentence.
const JOIN_BEFORE: &[&str] = &[
    "and", "but", "or", "so", "because", "which", "that", "who", "whom", "whose", "when", "while",
    "with", "to", "of", "for", "in", "on", "at", "is", "are", "was", "were", "be", "been", "being",
    "have", "has", "had", "can", "could", "will", "would", "should", "do", "does", "did", "not",
    "it", "we", "you", "they", "he", "she", "this", "that", "these", "those", "there", "here",
    "my", "our", "your", "their", "his", "her", "its",
];

const ABBREVIATIONS: &[&str] = &[
    "e.g", "i.e", "etc", "vs", "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "fig", "no",
    "approx", "dept", "est", "inc", "ltd", "co", "al", "cf", "p.m", "a.m",
];

const AUXILIARIES: &[&str] = &[
    "am", "is", "are", "was", "were", "be", "been", "being", "do", "does", "did", "have", "has",
    "had", "can", "could", "shall", "should", "will", "would", "may", "might", "must",
];

const WH_WORDS: &[&str] = &[
    "how", "what", "why", "when", "where", "who", "whom", "whose", "which",
];

fn join_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(\S+?)(\.)\s+(\p{Ll}[\p{L}']*)").expect("valid join regex"))
}

fn terminal_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"[^.!?]+[.!?]|[^.!?]+$").expect("valid terminal regex"))
}

fn bare_word(token: &str) -> String {
    token
        .trim_matches(|c: char| !c.is_alphanumeric() && c != '.')
        .to_lowercase()
}

fn is_punctuation(c: char) -> bool {
    matches!(c, ',' | '.' | '!' | '?' | ';' | ':')
}

fn is_starter(word: &str) -> bool {
    STARTERS.contains(&word.to_lowercase().as_str())
}

fn ends_with_abbreviation(text: &str) -> bool {
    let tail = text
        .trim_end_matches('.')
        .split_whitespace()
        .last()
        .unwrap_or("");
    ABBREVIATIONS.contains(&tail.to_lowercase().as_str())
}

/// Join a terminator that was almost certainly a pause, not a sentence end.
///
/// Conservative on purpose: both the word before and the word after must look
/// like a continuation, or the text is left alone.
pub fn repair_sentence_breaks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last = 0;
    for caps in join_re().captures_iter(text) {
        let Some(whole) = caps.get(0) else { continue };
        let before = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
        let terminator = caps.get(2).map(|m| m.as_str()).unwrap_or_default();
        let next = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
        let previous = bare_word(before);

        let abbreviation = ABBREVIATIONS.contains(&previous.as_str());
        let decimal =
            terminator == "." && before.chars().last().is_some_and(|c| c.is_ascii_digit());
        let proper_noun = before.chars().skip(1).any(|c| c.is_uppercase());
        let joins = !abbreviation
            && !decimal
            && !proper_noun
            && (JOIN_AFTER.contains(&previous.as_str())
                || JOIN_BEFORE.contains(&next.to_lowercase().as_str()));
        if !joins {
            continue;
        }
        out.push_str(&text[last..whole.start()]);
        out.push_str(before);
        out.push(' ');
        out.push_str(next);
        last = whole.end();
    }
    out.push_str(&text[last..]);
    out
}

fn reads_as_question(sentence: &str) -> bool {
    let words: Vec<String> = sentence
        .split_whitespace()
        .map(bare_word)
        .filter(|word| !word.is_empty())
        .collect();
    let Some(first) = words.first() else {
        return false;
    };
    if AUXILIARIES.contains(&first.as_str()) {
        return true;
    }
    if WH_WORDS.contains(&first.as_str()) {
        return words
            .get(1)
            .is_some_and(|second| AUXILIARIES.contains(&second.as_str()));
    }
    false
}

/// Whether `next` continues the sentence `previous` rather than starting a
/// new one. Used when VAD splits one spoken sentence into several segments.
pub fn continues_sentence(previous: &str, next: &str) -> bool {
    if next
        .split_whitespace()
        .next()
        .and_then(|word| word.chars().next())
        .is_some_and(|c| c.is_uppercase())
    {
        return false;
    }
    let next_word = next
        .split_whitespace()
        .next()
        .map(bare_word)
        .unwrap_or_default();
    let previous_word = previous
        .split_whitespace()
        .last()
        .map(bare_word)
        .unwrap_or_default();
    JOIN_AFTER.contains(&previous_word.as_str()) || JOIN_BEFORE.contains(&next_word.as_str())
}

/// Give a structurally interrogative sentence its question mark.
pub fn restore_question_marks(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for chunk in terminal_re().find_iter(text) {
        let sentence = chunk.as_str();
        let trimmed = sentence.trim_end();
        let has_terminal = matches!(trimmed.chars().last(), Some('.' | '?' | '!'));
        if trimmed.ends_with('.') && reads_as_question(trimmed) {
            out.push_str(&sentence[..sentence.len() - 1]);
            out.push('?');
        } else if !has_terminal && reads_as_question(trimmed) {
            out.push_str(trimmed);
            out.push('?');
        } else {
            out.push_str(sentence);
        }
    }
    out
}

fn normalize_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            if let Some(&next) = chars.get(i)
                && !is_punctuation(next)
                && !out.is_empty()
            {
                out.push(' ');
            }
            continue;
        }
        if is_punctuation(c) {
            while out.ends_with(' ') {
                out.pop();
            }
            out.push(c);
            if let Some(&next) = chars.get(i + 1) {
                let previous = i.checked_sub(1).and_then(|index| chars.get(index)).copied();
                let decimal = c == '.'
                    && previous.is_some_and(|p| p.is_ascii_digit())
                    && next.is_ascii_digit();
                let numeric_separator = matches!(c, ',' | ':')
                    && previous.is_some_and(|p| p.is_ascii_digit())
                    && next.is_ascii_digit();
                let closing = next.is_whitespace()
                    || is_punctuation(next)
                    || matches!(next, ')' | ']' | '}' | '"' | '\'');
                if !decimal && !numeric_separator && !closing {
                    out.push(' ');
                }
            }
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out.trim().to_string()
}

/// One space between words, none before punctuation, one after it. Decimal
/// points, thousands separators and clock times are preserved.
pub fn normalize_spacing(text: &str) -> String {
    text.split('\n')
        .map(normalize_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Capitalise the first word of a sentence when it is a common sentence
/// starter. Proper nouns and unknown lowercase words are left alone.
pub fn capitalize_sentences(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let mut sentence_start = true;
    while i < chars.len() {
        let c = chars[i];
        if c.is_alphabetic() {
            let start = i;
            while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '\'') {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            if sentence_start
                && word
                    .chars()
                    .next()
                    .is_some_and(|first| first.is_lowercase())
                && is_starter(&word)
            {
                let mut iter = word.chars();
                if let Some(first) = iter.next() {
                    out.extend(first.to_uppercase());
                    out.push_str(iter.as_str());
                }
            } else {
                out.push_str(&word);
            }
            sentence_start = false;
            continue;
        }
        out.push(c);
        if matches!(c, '.' | '?' | '!') {
            let decimal = c == '.'
                && i > 0
                && chars[i - 1].is_ascii_digit()
                && chars.get(i + 1).is_some_and(|next| next.is_ascii_digit());
            if !decimal && !ends_with_abbreviation(&out) {
                sentence_start = true;
            }
        }
        i += 1;
    }
    out
}

/// Add a final full stop to prose that the recogniser left open. Commands,
/// notes, one- or two-word utterances, and already-punctuated text are left
/// alone.
pub fn ensure_terminal_punctuation(text: &str) -> String {
    let trimmed = text.trim_end();
    if trimmed.is_empty() {
        return String::new();
    }
    let last = trimmed.chars().last().unwrap();
    if matches!(
        last,
        '.' | '!' | '?' | ',' | ';' | ':' | ')' | ']' | '}' | '"' | '\''
    ) {
        return trimmed.to_string();
    }
    if trimmed.split_whitespace().count() < 4 {
        return trimmed.to_string();
    }
    format!("{trimmed}.")
}

const NUMBER_WORDS: &[(&str, u64)] = &[
    ("zero", 0),
    ("one", 1),
    ("two", 2),
    ("three", 3),
    ("four", 4),
    ("five", 5),
    ("six", 6),
    ("seven", 7),
    ("eight", 8),
    ("nine", 9),
    ("ten", 10),
    ("eleven", 11),
    ("twelve", 12),
    ("thirteen", 13),
    ("fourteen", 14),
    ("fifteen", 15),
    ("sixteen", 16),
    ("seventeen", 17),
    ("eighteen", 18),
    ("nineteen", 19),
    ("twenty", 20),
    ("thirty", 30),
    ("forty", 40),
    ("fifty", 50),
    ("sixty", 60),
    ("seventy", 70),
    ("eighty", 80),
    ("ninety", 90),
    ("hundred", 100),
    ("thousand", 1_000),
    ("million", 1_000_000),
    ("billion", 1_000_000_000),
];

fn number_value(word: &str) -> Option<u64> {
    let word = word
        .trim_matches(|c: char| !c.is_alphanumeric())
        .to_lowercase();
    NUMBER_WORDS
        .iter()
        .find(|(name, _)| *name == word)
        .map(|(_, value)| *value)
}

/// Parse a run of number words (`twenty five`, `one hundred and five`) into a
/// value. Returns how many whitespace tokens were consumed.
fn parse_number_at(tokens: &[&str], start: usize) -> Option<(u64, usize)> {
    let mut total: u64 = 0;
    let mut current: u64 = 0;
    let mut used = 0;
    let mut saw_number = false;
    for (offset, token) in tokens.iter().enumerate().skip(start) {
        let cleaned = token.trim_matches(|c: char| !c.is_alphanumeric());
        if cleaned.eq_ignore_ascii_case("and") && saw_number {
            used = offset - start + 1;
            continue;
        }
        let Some(value) = number_value(cleaned) else {
            break;
        };
        saw_number = true;
        used = offset - start + 1;
        match value {
            100 => current = current.max(1) * 100,
            1_000.. => {
                total += current.max(1) * value;
                current = 0;
            }
            _ => current += value,
        }
    }
    saw_number.then_some((total + current, used))
}

fn trailing_suffix(token: &str) -> &str {
    token.trim_start_matches(|c: char| c.is_alphanumeric() || c == '.')
}

/// Turn dictated quantities into written forms. Narrow on purpose: a number
/// word without a currency, time, percent or unit marker is left as prose
/// ("one of the things" must not become "1 of the things").
pub fn format_numbers(text: &str) -> String {
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| {
            let tokens: Vec<&str> = line.split_whitespace().collect();
            let mut out: Vec<String> = Vec::with_capacity(tokens.len());
            let mut i = 0;
            while i < tokens.len() {
                let Some((value, used)) = parse_number_at(&tokens, i) else {
                    out.push(tokens[i].to_string());
                    i += 1;
                    continue;
                };
                let next = tokens.get(i + used).copied().unwrap_or("");
                let marker = next
                    .trim_matches(|c: char| !c.is_alphanumeric() && c != '.')
                    .to_lowercase();
                let suffix = trailing_suffix(next);
                match marker.as_str() {
                    "dollar" | "dollars" | "euro" | "euros" | "pound" | "pounds" => {
                        let symbol = match marker.as_str() {
                            "euro" | "euros" => "€",
                            "pound" | "pounds" => "£",
                            _ => "$",
                        };
                        // "and fifty cents" immediately after the currency.
                        let mut cents = 0;
                        let mut consumed = used + 1;
                        let has_and = tokens.get(i + consumed).is_some_and(|word| {
                            word.trim_matches(|c: char| !c.is_alphanumeric())
                                .eq_ignore_ascii_case("and")
                        });
                        let cents_parse = has_and
                            .then(|| parse_number_at(&tokens, i + consumed + 1))
                            .flatten();
                        if let Some((cent_value, cent_used)) = cents_parse {
                            let cent_word = tokens
                                .get(i + consumed + 1 + cent_used)
                                .copied()
                                .unwrap_or("");
                            if cent_word
                                .trim_matches(|c: char| !c.is_alphanumeric())
                                .eq_ignore_ascii_case("cents")
                                || cent_word
                                    .trim_matches(|c: char| !c.is_alphanumeric())
                                    .eq_ignore_ascii_case("cent")
                            {
                                cents = cent_value.min(99);
                                consumed += 2 + cent_used;
                            }
                        }
                        out.push(if cents > 0 {
                            format!("{symbol}{value}.{cents:02}{suffix}")
                        } else {
                            format!("{symbol}{value}{suffix}")
                        });
                        i += consumed;
                    }
                    "percent" => {
                        out.push(format!("{value}%{suffix}"));
                        i += used + 1;
                    }
                    "a.m." | "p.m." | "am" | "pm" | "o'clock" | "oclock" => {
                        // "three thirty" is 3:30, not the number thirty-three:
                        // read the hour and minute as separate fields.
                        let hour = number_value(tokens[i]).unwrap_or(0);
                        let minute = if used >= 2 {
                            number_value(tokens[i + 1]).unwrap_or(0)
                        } else {
                            0
                        };
                        if (1..=12).contains(&hour) && minute < 60 {
                            let marker = match marker.as_str() {
                                "am" => "a.m.".to_string(),
                                "pm" => "p.m.".to_string(),
                                other => other.to_string(),
                            };
                            if minute == 0 {
                                out.push(format!("{hour} {marker}"));
                            } else {
                                out.push(format!("{hour}:{minute:02} {marker}"));
                            }
                            i += used + 1;
                        } else {
                            out.push(tokens[i].to_string());
                            i += 1;
                        }
                    }
                    "minute" | "minutes" | "hour" | "hours" | "day" | "days" | "week" | "weeks"
                    | "month" | "months" | "year" | "years" | "megabyte" | "megabytes"
                    | "gigabyte" | "gigabytes" | "kilometre" | "kilometres" | "kilometer"
                    | "kilometers" | "mile" | "miles" | "degree" | "degrees" => {
                        out.push(format!("{value}{suffix}"));
                        i += used;
                    }
                    _ => {
                        out.push(tokens[i].to_string());
                        i += 1;
                    }
                }
            }
            out.join(" ")
        })
        .collect();
    lines.join("\n")
}

/// Spoken punctuation is opt-in. Replace only the whole word (ignoring
/// surrounding punctuation) so "comma" becomes a comma while "commander"
/// does not.
pub fn apply_spoken_punctuation(text: &str) -> String {
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let word = tokens[i]
            .trim_matches(|c: char| !c.is_alphanumeric())
            .to_lowercase();
        let pair = tokens.get(i + 1).map(|next| {
            next.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        });
        let (symbol, used) = match (word.as_str(), pair.as_deref()) {
            ("full", Some("stop")) | ("period", _) => (
                ".",
                if pair.as_deref() == Some("stop") {
                    2
                } else {
                    1
                },
            ),
            ("comma", _) => (",", 1),
            ("question", Some("mark")) => ("?", 2),
            ("exclamation", Some("mark")) | ("exclamation", Some("point")) => ("!", 2),
            ("colon", _) => (":", 1),
            ("semicolon", _) | ("semi", Some("colon")) => (";", if word == "semi" { 2 } else { 1 }),
            ("new", Some("line")) => ("\n", 2),
            ("new", Some("paragraph")) => ("\n\n", 2),
            ("open", Some("quote")) | ("quote", _) => ("\"", if word == "open" { 2 } else { 1 }),
            ("close", Some("quote")) => ("\"", 2),
            ("open", Some("parenthesis")) | ("open", Some("paren")) => ("(", 2),
            ("close", Some("parenthesis")) | ("close", Some("paren")) => (")", 2),
            ("dash", _) | ("hyphen", _) => ("-", 1),
            _ => {
                out.push(tokens[i].to_string());
                i += 1;
                continue;
            }
        };
        out.push(symbol.to_string());
        i += used;
    }
    out.join(" ")
}

/// Enforce dictionary spellings on the final transcript. Whole-word and
/// case-insensitive: "tauri" becomes "Tauri" and "Tauri app" can be replaced
/// as one phrase.
pub fn apply_replacements(text: &str, replacements: &[(String, String)]) -> String {
    let mut out = text.to_string();
    for (phrase, replacement) in replacements.iter().take(200) {
        if phrase.trim().is_empty() || phrase == replacement {
            continue;
        }
        let escaped = regex::escape(phrase.trim());
        let starts_word = phrase
            .trim()
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric());
        let ends_word = phrase
            .trim()
            .chars()
            .last()
            .is_some_and(|c| c.is_alphanumeric());
        let pattern = format!(
            "{}{}{}",
            if starts_word { r"\b" } else { "" },
            escaped,
            if ends_word { r"\b" } else { "" }
        );
        if let Ok(regex) = regex::RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
        {
            out = regex.replace_all(&out, replacement.as_str()).into_owned();
        }
    }
    out
}

/// The deterministic repair chain. No words are generated or removed.
pub fn normalize(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let numbered = format_numbers(text);
    let repaired = repair_sentence_breaks(&numbered);
    let questioned = restore_question_marks(&repaired);
    let spaced = normalize_spacing(&questioned);
    let cased = capitalize_sentences(&spaced);
    ensure_terminal_punctuation(&cased)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_a_premature_period_before_lowercase() {
        assert_eq!(
            repair_sentence_breaks("We fixed the. bug yesterday."),
            "We fixed the bug yesterday."
        );
        assert_eq!(
            repair_sentence_breaks("what are the phases. we have here."),
            "what are the phases we have here."
        );
    }

    #[test]
    fn leaves_abbreviations_and_decimals_alone() {
        assert_eq!(
            repair_sentence_breaks("Use e.g. this one."),
            "Use e.g. this one."
        );
        assert_eq!(
            repair_sentence_breaks("It costs 3.30 today."),
            "It costs 3.30 today."
        );
        assert_eq!(
            repair_sentence_breaks("The iPhone. app is here."),
            "The iPhone. app is here."
        );
        assert_eq!(
            repair_sentence_breaks("Hello. how are you?"),
            "Hello. how are you?"
        );
    }

    #[test]
    fn restores_question_marks_from_structure() {
        assert_eq!(restore_question_marks("what can we do."), "what can we do?");
        assert_eq!(
            restore_question_marks("can you hear me."),
            "can you hear me?"
        );
        assert_eq!(restore_question_marks("what we can do."), "what we can do.");
        assert_eq!(
            restore_question_marks("How are you. I am fine."),
            "How are you? I am fine."
        );
        assert_eq!(
            restore_question_marks("Hello. how are you"),
            "Hello. how are you?"
        );
    }

    #[test]
    fn fixes_punctuation_spacing_without_breaking_decimals() {
        assert_eq!(
            normalize_spacing("Hello , world .  How are you ?"),
            "Hello, world. How are you?"
        );
        assert_eq!(
            normalize_spacing("Version 3.30 is out."),
            "Version 3.30 is out."
        );
        assert_eq!(normalize_spacing("  a   b  "), "a b");
    }

    #[test]
    fn capitalizes_sentence_starts_but_not_inner_words() {
        assert_eq!(
            capitalize_sentences("what can we do? we can ship."),
            "What can we do? We can ship."
        );
        assert_eq!(
            capitalize_sentences("use the iPhone app."),
            "Use the iPhone app."
        );
        assert_eq!(capitalize_sentences("e.g. this one."), "e.g. this one.");
    }

    #[test]
    fn adds_a_terminal_full_stop_only_to_prose() {
        assert_eq!(
            ensure_terminal_punctuation("this is a complete sentence"),
            "this is a complete sentence."
        );
        assert_eq!(
            ensure_terminal_punctuation("one two three four."),
            "one two three four."
        );
        assert_eq!(ensure_terminal_punctuation("git status"), "git status");
        assert_eq!(ensure_terminal_punctuation("note:"), "note:");
    }

    #[test]
    fn formats_money_time_percent_and_units() {
        assert_eq!(format_numbers("twenty five dollars"), "$25");
        assert_eq!(
            format_numbers("twenty five dollars and fifty cents"),
            "$25.50"
        );
        assert_eq!(format_numbers("three thirty p.m."), "3:30 p.m.");
        assert_eq!(format_numbers("ten percent"), "10%");
        assert_eq!(format_numbers("five minutes"), "5 minutes");
    }

    #[test]
    fn leaves_bare_number_words_as_prose() {
        assert_eq!(format_numbers("one of the things"), "one of the things");
        assert_eq!(
            format_numbers("version three point thirty"),
            "version three point thirty"
        );
    }

    #[test]
    fn replacements_are_whole_word_and_case_insensitive() {
        let replacements = vec![
            ("tauri".to_string(), "GPUI".to_string()),
            ("open router".to_string(), "OpenRouter".to_string()),
        ];
        assert_eq!(
            apply_replacements("the Tauri app uses open router", &replacements),
            "the GPUI app uses OpenRouter"
        );
        assert_eq!(
            apply_replacements("taurine is not tauri", &replacements),
            "taurine is not GPUI"
        );
    }

    #[test]
    fn spoken_punctuation_is_opt_in_and_word_exact() {
        assert_eq!(
            apply_spoken_punctuation("hello comma world period"),
            "hello , world ."
        );
        assert_eq!(
            apply_spoken_punctuation("open quote ship it close quote"),
            "\" ship it \""
        );
        assert_eq!(apply_spoken_punctuation("commander comma"), "commander ,");
        assert_eq!(
            apply_spoken_punctuation("first line new line second"),
            "first line \n second"
        );
    }

    #[test]
    fn continuation_words_join_across_vad_segments() {
        assert!(continues_sentence(
            "So here is the testing.",
            "of the application I'm running."
        ));
        assert!(continues_sentence(
            "Currently I am not understanding what",
            "can I do?"
        ));
        assert!(!continues_sentence(
            "See you can see after testing there was a full stop.",
            "then it should have capitalized the O."
        ));
        assert!(!continues_sentence("Mm-hmm.", "So here is the testing."));
    }

    #[test]
    fn normalize_runs_the_whole_chain() {
        assert_eq!(
            normalize("what can we do. we can ship"),
            "What can we do we can ship?"
        );
        assert_eq!(
            normalize("hello , world . how are you ?"),
            "Hello, world. How are you?"
        );
        assert_eq!(normalize("hello. how are you"), "Hello. How are you?");
    }
}
