use super::*;

fn b(s: &str) -> Bytes {
    Bytes::copy_from_slice(s.as_bytes())
}

#[test]
fn decodes_single_event() {
    let mut d = SseDecoder::new();
    let evs = d.feed(b("data: {\"a\":1}\n\n"));
    assert_eq!(evs, vec![SseEvent::Data("{\"a\":1}".into())]);
}

#[test]
fn decodes_done_sentinel() {
    let mut d = SseDecoder::new();
    let evs = d.feed(b("data: [DONE]\n\n"));
    assert_eq!(evs, vec![SseEvent::Done]);
}

#[test]
fn handles_split_chunks() {
    let mut d = SseDecoder::new();
    assert!(d.feed(b("data: {\"a\":")).is_empty());
    assert!(d.feed(b("1}")).is_empty());
    let evs = d.feed(b("\n\n"));
    assert_eq!(evs, vec![SseEvent::Data("{\"a\":1}".into())]);
}

#[test]
fn ignores_comments_and_unknown_fields() {
    let mut d = SseDecoder::new();
    let evs = d.feed(b(": ping\nevent: chunk\ndata: hi\n\n"));
    assert_eq!(evs, vec![SseEvent::Data("hi".into())]);
}

#[test]
fn handles_crlf_terminators() {
    let mut d = SseDecoder::new();
    let evs = d.feed(b("data: hi\r\n\r\n"));
    assert_eq!(evs, vec![SseEvent::Data("hi".into())]);
}

#[test]
fn dispatches_multiple_events_in_one_feed() {
    let mut d = SseDecoder::new();
    let evs = d.feed(b("data: a\n\ndata: b\n\ndata: [DONE]\n\n"));
    assert_eq!(
        evs,
        vec![
            SseEvent::Data("a".into()),
            SseEvent::Data("b".into()),
            SseEvent::Done
        ]
    );
}
