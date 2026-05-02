use super::*;

#[test]
fn parses_text_delta_chunk() {
    let raw = r#"{"id":"chatcmpl-1","object":"chat.completion.chunk","choices":[{"index":0,"delta":{"role":"assistant","content":"Hi"},"finish_reason":null}]}"#;
    let ev: ChatStreamEvent = serde_json::from_str(raw).unwrap();
    assert_eq!(ev.id, "chatcmpl-1");
    assert_eq!(ev.choices[0].delta.content.as_deref(), Some("Hi"));
}

#[test]
fn parses_tool_call_delta() {
    let raw = r#"{"id":"x","choices":[{"index":0,"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"grep","arguments":"{\"q\":"}}]},"finish_reason":null}]}"#;
    let ev: ChatStreamEvent = serde_json::from_str(raw).unwrap();
    let tc = &ev.choices[0].delta.tool_calls.as_ref().unwrap()[0];
    assert_eq!(tc.id.as_deref(), Some("call_1"));
    assert_eq!(tc.function.as_ref().unwrap().name.as_deref(), Some("grep"));
}

#[test]
fn parses_finish_with_usage() {
    let raw = r#"{"id":"x","choices":[{"index":0,"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":10,"completion_tokens":5,"total_tokens":15}}"#;
    let ev: ChatStreamEvent = serde_json::from_str(raw).unwrap();
    assert_eq!(ev.choices[0].finish_reason, Some(FinishReason::Stop));
    assert_eq!(ev.usage.as_ref().unwrap().completion_tokens, 5);
}

#[test]
fn ignores_unknown_top_level_fields() {
    let raw =
        r#"{"id":"x","object":"chat.completion.chunk","system_fingerprint":"fp","choices":[]}"#;
    let _: ChatStreamEvent = serde_json::from_str(raw).unwrap();
}

#[test]
fn serializes_request_skips_none() {
    let req = ChatRequest {
        model: "gpt-4o".into(),
        messages: vec![ChatMessage::User {
            content: UserContent::Text("hi".into()),
        }],
        tools: None,
        tool_choice: None,
        max_completion_tokens: None,
        temperature: None,
        stream: true,
        stream_options: Some(StreamOptions {
            include_usage: true,
        }),
    };
    let s = serde_json::to_string(&req).unwrap();
    assert!(!s.contains("tools"));
    assert!(!s.contains("temperature"));
    assert!(s.contains("\"stream\":true"));
}
