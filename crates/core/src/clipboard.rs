use crate::Result;

pub fn read_text() -> Option<String> {
    arboard::Clipboard::new()
        .ok()?
        .get_text()
        .ok()
        .filter(|s| !s.is_empty())
}

pub fn write_text(text: &str) -> Result<()> {
    arboard::Clipboard::new()
        .map_err(|e| crate::Error::Message(e.to_string()))?
        .set_text(text)
        .map_err(|e| crate::Error::Message(e.to_string()))
}
