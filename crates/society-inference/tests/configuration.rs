//! Provider configuration safety and URL validation.
use society_inference::{InferenceError, OpenAiCompatibleProvider, ProviderConfig};

#[test]
fn accepts_loopback_http_and_remote_https() {
    for url in [
        "http://localhost:1234/v1",
        "http://127.0.0.1:1234/v1",
        "http://[::1]:1234/v1",
        "https://example.com/v1",
    ] {
        let config = ProviderConfig {
            base_url: url.into(),
            ..ProviderConfig::default()
        };
        assert!(OpenAiCompatibleProvider::new(config).is_ok(), "{url}");
    }
}

#[test]
fn rejects_insecure_or_ambiguous_provider_urls() {
    for url in [
        "http://example.com/v1",
        "ftp://localhost/v1",
        "https://user:password@example.com/v1",
        "https://example.com/v1?key=secret",
        "https://example.com/v1#fragment",
        "not a url",
    ] {
        let config = ProviderConfig {
            base_url: url.into(),
            ..ProviderConfig::default()
        };
        assert!(
            matches!(
                OpenAiCompatibleProvider::new(config),
                Err(InferenceError::Configuration(_))
            ),
            "{url}"
        );
    }
}
