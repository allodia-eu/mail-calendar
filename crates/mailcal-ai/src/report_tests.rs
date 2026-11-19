use super::refusal_summary;

#[test]
fn an_openai_shaped_refusal_gives_its_type_code_parameter_and_message() {
    let body = r#"{"error":{"message":"Invalid schema for function 'emit_json': 'uint8' is not a valid format.","type":"invalid_request_error","param":"tools[0].function.parameters","code":"invalid_function_parameters"}}"#;
    assert_eq!(
        refusal_summary(body),
        "type=invalid_request_error code=invalid_function_parameters \
         param=tools[0].function.parameters message=\"Invalid schema for function 'emit_json': \
         'uint8' is not a valid format.\""
    );
}

#[test]
fn a_refusal_with_its_fields_at_the_top_is_read_too() {
    let body = r#"{"object":"error","message":"max_tokens is too large","type":"BadRequestError","param":null,"code":400}"#;
    assert_eq!(
        refusal_summary(body),
        "type=BadRequestError code=400 message=\"max_tokens is too large\""
    );
}

#[test]
fn a_detail_string_is_the_message() {
    assert_eq!(
        refusal_summary(r#"{"detail":"Model not found"}"#),
        "message=\"Model not found\""
    );
}

#[test]
fn the_relay_envelope_gives_its_own_code() {
    let body = r#"{"defined":true,"code":"FORBIDDEN","status":403,"message":"Forbidden","data":{"code":"not_entitled"}}"#;
    assert_eq!(
        refusal_summary(body),
        "code=not_entitled message=\"Forbidden\""
    );
}

#[test]
fn a_long_message_is_cut_and_kept_on_one_line() {
    let long = format!("first line\n{}", "x".repeat(400));
    let body = serde_json::json!({ "error": { "message": long } }).to_string();
    let summary = refusal_summary(&body);
    assert!(!summary.contains('\n'));
    assert!(summary.starts_with("message=\"first line xxx"));
    assert!(summary.ends_with("…\""));
    assert!(summary.chars().count() < 230);
}

#[test]
fn an_answer_that_is_not_json_is_described_by_its_size_only() {
    let html = "<html><body>Bad Gateway: upstream api.example.eu</body></html>";
    let summary = refusal_summary(html);
    assert_eq!(
        summary,
        format!("a non-JSON answer of {} bytes", html.len())
    );
    assert!(!summary.contains("example"));
}

#[test]
fn json_without_error_fields_is_described_by_its_size_only() {
    assert_eq!(
        refusal_summary(r#"{"choices":[]}"#),
        "no error fields in 14 bytes"
    );
}

#[test]
fn a_field_that_is_not_a_short_word_is_left_out() {
    let body = r#"{"error":{"type":"a type with spaces that goes on","code":{"nested":1},"message":"no"}}"#;
    assert_eq!(refusal_summary(body), "message=\"no\"");
}
