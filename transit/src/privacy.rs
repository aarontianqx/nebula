pub fn scrub(value: &str, secrets: &[String], limit: usize) -> String {
    let mut text = value.chars().take(4096).collect::<String>();
    for secret in secrets {
        text = text.replace(secret, "[redacted]");
    }
    let lower = text.to_ascii_lowercase();
    if text.contains(['\n', '\r', '`', '{', '}'])
        || [
            "<|fim_",
            "bearer ",
            "prompt:",
            "prompt=",
            "completion:",
            "content:",
            "source code",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
    {
        return "[sensitive error detail omitted]".into();
    }
    // Remove quoted fragments, since providers sometimes echo user input in errors.
    let mut quoted = None;
    let mut safe = String::new();
    for character in text.chars() {
        if let Some(delimiter) = quoted {
            if character == delimiter {
                quoted = None;
            }
            continue;
        }
        if character == '\'' || character == '"' {
            quoted = Some(character);
            safe.push_str("[quoted detail omitted]");
        } else {
            safe.push(character);
        }
    }
    safe.split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if word.contains("://")
                || word.contains(['?', '=', '\\'])
                || word.len() > 96
                || [
                    "sk-",
                    "key-",
                    "api_key",
                    "api-key",
                    "token:",
                    "authorization:",
                ]
                .iter()
                .any(|marker| lower.contains(marker))
            {
                "[redacted]"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(limit)
        .collect()
}
