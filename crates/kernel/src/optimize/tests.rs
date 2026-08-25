use super::*;

#[test]
fn empty_and_singleton_are_unchanged() {
    let empty = Toolpath {
        version: 0,
        meta: None,
        segments: vec![],
    };
    assert_eq!(merge_collinear(&empty), empty);
}
