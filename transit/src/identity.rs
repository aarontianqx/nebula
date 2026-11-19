use axum::http::{HeaderMap, header};
use sha2::{Digest, Sha256};

pub fn credential_hash(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    let mut values = Vec::new();
    for value in headers.get_all(header::AUTHORIZATION) {
        if let Some(value) = value.to_str().ok().and_then(bearer_token) {
            values.push(value.to_string());
        }
    }
    for name in ["x-api-key", "api-key", "x-goog-api-key"] {
        for value in headers.get_all(name) {
            if let Some(value) = value.to_str().ok().filter(|value| valid_key(value)) {
                values.push(value.to_string());
            }
        }
    }
    values.extend(
        crate::admission::query_keys(query)
            .filter(|value| valid_key(value))
            .map(|value| value.into_owned()),
    );
    values.sort_unstable();
    values.dedup();
    if values.len() != 1 {
        return None;
    }
    Some(format!("{:x}", Sha256::digest(values[0].as_bytes())))
}

fn bearer_token(value: &str) -> Option<&str> {
    let (scheme, token) = value.split_once(' ')?;
    (scheme.eq_ignore_ascii_case("bearer") && valid_key(token)).then_some(token)
}

fn valid_key(value: &str) -> bool {
    !value.is_empty() && !value.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_one_canonical_credential_without_retaining_the_secret() {
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer abc".parse().unwrap());
        let hash = credential_hash(&headers, None).unwrap();
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            credential_hash(&headers, Some("key=abc")).as_deref(),
            Some(hash.as_str())
        );
    }

    #[test]
    fn accepts_supported_header_schemes_and_query_encoding() {
        for (name, value) in [
            ("authorization", "bEaReR fixture-secret"),
            ("x-api-key", "fixture-secret"),
            ("api-key", "fixture-secret"),
            ("x-goog-api-key", "fixture-secret"),
        ] {
            let mut headers = HeaderMap::new();
            headers.insert(name, value.parse().unwrap());
            assert_eq!(
                credential_hash(&headers, None),
                credential_hash(&HeaderMap::new(), Some("key=fixture%2Dsecret"))
            );
        }
    }

    #[test]
    fn missing_invalid_or_conflicting_credentials_have_no_identity() {
        assert_eq!(credential_hash(&HeaderMap::new(), None), None);
        for value in ["Basic abc", "Bearer ", "Bearer two words", "abc"] {
            let mut headers = HeaderMap::new();
            headers.insert(header::AUTHORIZATION, value.parse().unwrap());
            assert_eq!(credential_hash(&headers, None), None, "{value}");
        }
        let mut headers = HeaderMap::new();
        headers.insert(header::AUTHORIZATION, "Bearer first".parse().unwrap());
        headers.insert("x-api-key", "second".parse().unwrap());
        assert_eq!(credential_hash(&headers, None), None);
        assert_eq!(
            credential_hash(&HeaderMap::new(), Some("key=first&key=second")),
            None
        );
    }
}
