//! URL query parsing with percent-decoding.

use std::collections::BTreeMap;

/// Parses `a=b&c=d%3De` into a map; later keys win. Keys and values are decoded.
pub(crate) fn parse(query: Option<&str>) -> BTreeMap<String, String> {
    query
        .unwrap_or_default()
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(key), decode(value))
        })
        .collect()
}

fn decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'+' => out.push(b' '),
            b'%' if index + 2 < bytes.len() => {
                let hex = &text[index + 1..index + 3];
                match u8::from_str_radix(hex, 16) {
                    Ok(byte) => {
                        out.push(byte);
                        index += 2;
                    },
                    Err(_) => out.push(b'%'),
                }
            },
            byte => out.push(byte),
        }
        index += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::parse;

    #[test]
    fn decodes_percent_and_plus() {
        let params = parse(Some(
            "watch=true&fieldSelector=metadata.name%3Dhello&resourceVersion=42&x=a+b&bad=%zz",
        ));
        assert_eq!(params["watch"], "true");
        assert_eq!(params["fieldSelector"], "metadata.name=hello");
        assert_eq!(params["resourceVersion"], "42");
        assert_eq!(params["x"], "a b");
        assert_eq!(params["bad"], "%zz");
        assert!(parse(None).is_empty());
    }
}
