//! Testable policy primitives for the Linux WebKitGTK hosts.

/// WebKit content-filter JSON that blocks every HTTP(S) resource before it leaves the process.
pub(crate) fn network_block_filter() -> &'static str {
    r#"[{"trigger":{"url-filter":"^https?://"},"action":{"type":"block"}}]"#
}

/// The only navigation WebKit may commit inside either in-memory content island.
pub(crate) fn is_initial_document_uri(uri: Option<&str>) -> bool {
    uri == Some("about:blank")
}

/// Sanitises an attachment suffix used for an app-owned temporary file.
pub(crate) fn safe_extension(file_name: &str) -> String {
    let Some(extension) = std::path::Path::new(file_name)
        .extension()
        .and_then(|extension| extension.to_str())
    else {
        return ".bin".to_owned();
    };
    if extension.is_empty()
        || extension.len() > 12
        || !extension.bytes().all(|byte| byte.is_ascii_alphanumeric())
    {
        ".bin".to_owned()
    } else {
        format!(".{extension}")
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;

    use super::{is_initial_document_uri, network_block_filter, safe_extension};

    #[test]
    fn native_filter_blocks_every_http_and_https_resource() {
        let rules: Value = serde_json::from_str(network_block_filter()).expect("valid filter JSON");
        let rule = &rules.as_array().expect("rule array")[0];

        assert_eq!(rule["trigger"]["url-filter"], "^https?://");
        assert_eq!(rule["action"]["type"], "block");
    }

    #[test]
    fn initial_load_allowance_cannot_be_consumed_by_an_external_link() {
        assert!(is_initial_document_uri(Some("about:blank")));
        assert!(!is_initial_document_uri(Some("https://example.test")));
        assert!(!is_initial_document_uri(Some("mailto:user@example.test")));
        assert!(!is_initial_document_uri(None));
    }

    #[test]
    fn temporary_attachment_extensions_are_ascii_and_bounded() {
        assert_eq!(safe_extension("report.final.pdf"), ".pdf");
        assert_eq!(safe_extension("report.final draft"), ".bin");
        assert_eq!(safe_extension("archive.tar.gz"), ".gz");
        assert_eq!(safe_extension("dangerous.sh; touch nope"), ".bin");
        assert_eq!(safe_extension("no-extension"), ".bin");
        assert_eq!(safe_extension("image.verylongextension"), ".bin");
    }
}
