pub fn synchronize_punctuation(base: &str, reference: &str) -> String {
    if base.trim().is_empty() || reference.trim().is_empty() {
        return base.to_string();
    }

    let trimmed = base.trim().trim_end_matches([
        '.', '?', '!', ';', ':', ',', '"', '\'', ' ', '\u{201d}', '\u{2019}',
    ]);
    let reference = reference
        .trim()
        .trim_end_matches(['"', '\'', ' ', '\u{201d}', '\u{2019}']);

    if reference.ends_with('?') {
        format!("{trimmed}?")
    } else if reference.ends_with('!') {
        format!("{trimmed}!")
    } else if reference.ends_with('.') {
        format!("{trimmed}.")
    } else {
        trimmed.to_string()
    }
}

pub fn comma_before_question(text: &str) -> String {
    let mut output = String::with_capacity(text.len() + 4);
    let chars = text.chars();

    for current in chars {
        if current == '?' {
            let trimmed = output.trim_end();
            if !trimmed.ends_with(',') && !trimmed.is_empty() {
                while output.ends_with(char::is_whitespace) {
                    output.pop();
                }
                output.push_str(", ?");
            } else {
                output.push('?');
            }
        } else {
            output.push(current);
        }
    }

    output
}

pub fn correct_ptbr_pronunciation(text: &str) -> String {
    let replacements = [
        ("olho", "ólho"),
        ("posso", "pósso"),
        ("jogo", "jógo"),
        ("gosto", "gósto"),
        ("fora", "fóra"),
        ("agora", "agóra"),
        ("por", "pór"),
        ("milha", "mílha"),
    ];

    text.split_whitespace()
        .map(|token| {
            let lower = token.to_lowercase();
            replacements
                .iter()
                .find_map(|(from, to)| (lower == *from).then_some((*to).to_string()))
                .unwrap_or_else(|| token.to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn palatalize_ptbr(text: &str) -> String {
    text.split_whitespace()
        .map(palatalize_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn palatalize_token(token: &str) -> String {
    if is_bracketed_control_token(token) {
        return token.to_string();
    }

    let suffixes = [
        ("tis", "tchis"),
        ("tes", "tches"),
        ("ti", "tchi"),
        ("te", "tche"),
        ("dis", "dchis"),
        ("des", "dches"),
        ("di", "dchi"),
        ("de", "dche"),
    ];

    for (from, to) in suffixes {
        if token.len() > from.len() && token.ends_with(from) {
            let prefix = &token[..token.len() - from.len()];
            return format!("{prefix}{to}");
        }

        let capitalized = capitalize_ascii(from);
        if token.len() > capitalized.len() && token.ends_with(&capitalized) {
            let prefix = &token[..token.len() - capitalized.len()];
            return format!("{prefix}{}", capitalize_ascii(to));
        }
    }

    token.to_string()
}

fn is_bracketed_control_token(token: &str) -> bool {
    token.starts_with('[') && token.ends_with(']')
}

fn capitalize_ascii(value: &str) -> String {
    let mut chars = value.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_ascii_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synchronizes_final_punctuation_from_reference() {
        assert_eq!(
            synchronize_punctuation("Ola mundo.", "Hello?"),
            "Ola mundo?"
        );
        assert_eq!(synchronize_punctuation("Ola mundo", "Hello!"), "Ola mundo!");
        assert_eq!(
            synchronize_punctuation("Ola mundo?", "Hello."),
            "Ola mundo."
        );
    }

    #[test]
    fn inserts_comma_before_question_mark() {
        assert_eq!(comma_before_question("Tudo bem?"), "Tudo bem, ?");
        assert_eq!(comma_before_question("Tudo bem, ?"), "Tudo bem, ?");
    }

    #[test]
    fn palatalizes_ptbr_suffixes_without_touching_isolated_words() {
        assert_eq!(
            palatalize_ptbr("bati noite pedi mode"),
            "batchi noitche pedchi modche"
        );
        assert_eq!(palatalize_ptbr("de te di ti"), "de te di ti");
    }

    #[test]
    fn keeps_bracketed_omnivoice_tags_unchanged() {
        assert_eq!(
            palatalize_ptbr("[question-en] bati [surprise-oh]"),
            "[question-en] batchi [surprise-oh]"
        );
    }
}
