use std::fs;

use tempfile::tempdir;
use txtbatch::{process_dir, Operation, DEFAULT_CTX};

#[test]
fn replace_writes_files_recursively() {
    let tmp = tempdir().unwrap();
    fs::create_dir(tmp.path().join("sub")).unwrap();
    fs::write(tmp.path().join("a.txt"), "foo bar\nfoo").unwrap();
    fs::write(tmp.path().join("sub/b.txt"), "nothing").unwrap();

    let op = Operation::Replace {
        find: "foo".into(),
        replace: "FOO".into(),
    };
    let s = process_dir(tmp.path(), &op, false, DEFAULT_CTX).unwrap();

    assert_eq!(s.files_scanned, 2);
    assert_eq!(s.files_modified, 1);
    assert_eq!(s.total_edits, 2);
    assert_eq!(s.unmatched.len(), 1);
    assert_eq!(s.errors.len(), 0);
    assert_eq!(
        fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
        "FOO bar\nFOO"
    );
    assert_eq!(fs::read_to_string(tmp.path().join("sub/b.txt")).unwrap(), "nothing");
}

#[test]
fn dry_run_does_not_write() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), "foo").unwrap();

    let op = Operation::Replace {
        find: "foo".into(),
        replace: "bar".into(),
    };
    let s = process_dir(tmp.path(), &op, true, DEFAULT_CTX).unwrap();

    assert_eq!(s.files_modified, 1);
    assert_eq!(s.total_edits, 1);
    assert_eq!(fs::read_to_string(tmp.path().join("a.txt")).unwrap(), "foo");
}

#[test]
fn insert_end_to_end() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), "你好世界").unwrap();

    let op = Operation::Insert {
        after: "你好".into(),
        insert: " ".into(),
    };
    process_dir(tmp.path(), &op, false, DEFAULT_CTX).unwrap();

    assert_eq!(
        fs::read_to_string(tmp.path().join("a.txt")).unwrap(),
        "你好 世界"
    );
}

#[test]
fn binary_files_skipped_and_counted() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("bin.png"), [0u8, 1, 2, 3]).unwrap();
    fs::write(tmp.path().join("text.txt"), "foo foo").unwrap();

    let op = Operation::Replace {
        find: "foo".into(),
        replace: "x".into(),
    };
    let s = process_dir(tmp.path(), &op, false, DEFAULT_CTX).unwrap();

    assert_eq!(s.binary_skipped, 1);
    assert_eq!(s.files_scanned, 2);
    assert_eq!(s.files_modified, 1);
    assert_eq!(s.total_edits, 2);
    assert_eq!(s.modified.len(), 1);
}

#[test]
fn no_match_files_reported() {
    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), "hello").unwrap();

    let op = Operation::Replace {
        find: "zzz".into(),
        replace: "x".into(),
    };
    let s = process_dir(tmp.path(), &op, false, DEFAULT_CTX).unwrap();

    assert_eq!(s.files_modified, 0);
    assert_eq!(s.unmatched.len(), 1);
    assert_eq!(fs::read_to_string(tmp.path().join("a.txt")).unwrap(), "hello");
}

#[test]
fn empty_directory() {
    let tmp = tempdir().unwrap();
    let op = Operation::Replace {
        find: "a".into(),
        replace: "b".into(),
    };
    let s = process_dir(tmp.path(), &op, false, DEFAULT_CTX).unwrap();
    assert_eq!(s.files_scanned, 0);
    assert_eq!(s.files_modified, 0);
}

#[test]
fn missing_dir_errors() {
    let tmp = tempdir().unwrap();
    let missing = tmp.path().join("nope");
    let op = Operation::Replace {
        find: "a".into(),
        replace: "b".into(),
    };
    assert!(process_dir(&missing, &op, false, DEFAULT_CTX).is_err());
}

#[test]
fn file_path_errors() {
    let tmp = tempdir().unwrap();
    let f = tmp.path().join("a.txt");
    fs::write(&f, "hi").unwrap();
    let op = Operation::Replace {
        find: "a".into(),
        replace: "b".into(),
    };
    assert!(process_dir(&f, &op, false, DEFAULT_CTX).is_err());
}