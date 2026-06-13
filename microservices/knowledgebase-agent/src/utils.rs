/// Truncate text at a character boundary, appending "..." if truncated.
pub fn truncate_at_char(text: &str, max_chars: usize) -> String {
    if text.len() <= max_chars {
        // Fast path: if byte length is within limit, char length definitely is too
        return text.to_string();
    }
    match text.char_indices().nth(max_chars) {
        Some((i, _)) => format!("{}...", &text[..i]),
        None => text.to_string(),
    }
}
