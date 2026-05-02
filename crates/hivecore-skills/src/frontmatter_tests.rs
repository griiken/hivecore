use super::*;

#[test]
fn splits_yaml_and_body() {
    let raw = "---\nname: x\ndescription: hi\n---\nbody here\nline 2\n";
    let (fm, body) = split(raw, "x.md").unwrap();
    assert_eq!(fm, Some("name: x\ndescription: hi\n"));
    assert_eq!(body, "body here\nline 2\n");
}

#[test]
fn handles_no_frontmatter() {
    let raw = "just a body\n";
    let (fm, body) = split(raw, "x.md").unwrap();
    assert_eq!(fm, None);
    assert_eq!(body, "just a body\n");
}

#[test]
fn rejects_unterminated_frontmatter() {
    let raw = "---\nname: x\nno closer";
    let err = split(raw, "x.md").unwrap_err();
    assert!(format!("{err}").contains("never closed"));
}

#[test]
fn handles_crlf() {
    let raw = "---\r\nname: x\r\n---\r\nbody\r\n";
    let (fm, body) = split(raw, "x.md").unwrap();
    assert_eq!(fm, Some("name: x\r\n"));
    assert_eq!(body, "body\r\n");
}

#[test]
fn strips_bom() {
    let raw = "\u{FEFF}---\nname: x\n---\nbody\n";
    let (fm, body) = split(raw, "x.md").unwrap();
    assert!(fm.is_some());
    assert_eq!(body, "body\n");
}
