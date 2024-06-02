use std::collections::HashMap;
use std::borrow::Cow;

pub fn format_environment_for_docker(env: &HashMap<String, serde_json::Value>) -> Vec<String> {
    env.iter().map(|(k, v)| {
        let value = v.as_str()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(format!("{v}")));

        format!("{k}={value}")
    }).collect()
}

/// Sanitizes the given bytes to remove bad control characters
pub fn sanitize_output(bytes: &[u8]) -> String {
    // would be better if it didn't strip colors and stuff but oh well

    // strip controls except whitespaces
    String::from_utf8_lossy(bytes)
        .as_ref()
        .chars()
        // filter if
        //   - is a replacement char
        //   - is a non-whitespace control
        .filter(|c| c != &char::REPLACEMENT_CHARACTER && (!c.is_control() || c.is_whitespace()))
        .collect::<String>()
}

