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
    RE.get_or_init(|| Regex::new(r"(\S+?)([.!?])\s+(\p{Ll}[\p{L}']*)").expect("valid join regex"))
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

/// The deterministic repair chain. No words are generated or removed.
pub fn normalize(text: &str) -> String {
    if text.trim().is_empty() {
        return String::new();
    }
    let repaired = repair_sentence_breaks(text);
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
