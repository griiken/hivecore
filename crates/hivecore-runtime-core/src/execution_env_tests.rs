//! Trait-shape sanity tests. Real impl coverage lives in
//! `hivecore-execution-env` and `hivecore-builtin-tools`.

use super::*;

#[test]
fn exec_output_is_success() {
    let ok = ExecOutput {
        stdout: vec![],
        stderr: vec![],
        exit_code: 0,
    };
    let fail = ExecOutput {
        stdout: vec![],
        stderr: vec![],
        exit_code: 1,
    };
    assert!(ok.is_success());
    assert!(!fail.is_success());
}

#[test]
fn exec_opts_default_empty() {
    let o = ExecOpts::default();
    assert!(o.cwd.is_none());
    assert!(o.env.is_empty());
    assert!(o.timeout.is_none());
    assert!(o.signal.is_none());
}

#[test]
fn remove_opts_default_safe() {
    let o = RemoveOpts::default();
    assert!(!o.recursive);
    assert!(!o.force);
}
