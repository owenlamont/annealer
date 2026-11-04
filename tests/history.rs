#![forbid(unsafe_code)]
#![deny(clippy::all, clippy::pedantic)]

use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

type Delta = (String, String, String, String);

fn git(path: &Path, args: &[&str]) -> Vec<u8> {
    let output = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_AUTHOR_NAME", "Raw Author")
        .env("GIT_AUTHOR_EMAIL", "author@example.invalid")
        .env("GIT_COMMITTER_NAME", "Other Committer")
        .env("GIT_COMMITTER_EMAIL", "committer@example.invalid")
        .env("GIT_AUTHOR_DATE", "1700000000 +0530")
        .env("GIT_COMMITTER_DATE", "1700000060 -0700")
        .output()
        .expect("Git can start");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn commit(path: &Path, message: &str) {
    git(path, &["add", "-A"]);
    git(
        path,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "-m",
            message,
        ],
    );
}

fn inspect(path: &Path, revision: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_annealer"))
        .arg("inspect")
        .arg(path)
        .args(["--revision", revision])
        .output()
        .expect("inspection can start");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("inspection is JSON")
}

fn bytes(value: &Value) -> Vec<u8> {
    serde_json::from_value(value.clone()).expect("raw bytes are an integer array")
}

fn fixture(path: &Path) {
    git(path, &["init", "--initial-branch=main"]);
    fs::write(path.join("text.txt"), b"one\ntwo\nthree\n").unwrap();
    fs::write(path.join("binary.bin"), b"\0binary\xff").unwrap();
    fs::write(path.join("type"), b"file\n").unwrap();
    fs::write(path.join("space name.txt"), b"raw\n").unwrap();
    fs::create_dir(path.join("folder")).unwrap();
    fs::write(path.join("folder/nested.txt"), b"nested\n").unwrap();
    fs::write(path.join("folder/empty"), b"").unwrap();
    #[cfg(unix)]
    {
        fs::write(path.join("tab\tname.txt"), b"raw\n").unwrap();
    }
    // The macOS runner's filesystem rejects non-UTF-8 filenames.
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use std::os::unix::ffi::OsStrExt;
        fs::write(
            path.join(std::ffi::OsStr::from_bytes(b"nonutf8-\xff\n")),
            b"raw\n",
        )
        .unwrap();
    }
    commit(path, "root");
    git(path, &["tag", "root"]);
    commit(path, "empty");
    git(path, &["checkout", "-b", "side"]);
    fs::write(path.join("side.txt"), b"branch\n").unwrap();
    commit(path, "side");
    git(path, &["checkout", "main"]);
    fs::rename(path.join("text.txt"), path.join("renamed.txt")).unwrap();
    fs::rename(path.join("folder"), path.join("moved")).unwrap();
    fs::remove_file(path.join("type")).unwrap();
    fs::create_dir(path.join("type")).unwrap();
    fs::write(path.join("type/child"), b"child\n").unwrap();
    commit(path, "rename and type");
    git(
        path,
        &[
            "-c",
            "commit.gpgsign=false",
            "merge",
            "--no-ff",
            "side",
            "-m",
            "merge",
        ],
    );
    fs::write(path.join("renamed.txt"), b"one\nchanged\nthree\nfour\n").unwrap();
    fs::remove_file(path.join("binary.bin")).unwrap();
    commit(path, "text edit and binary removal");
    git(path, &["tag", "inspected"]);
}

fn native_deltas(path: &Path, event: &Value) -> BTreeMap<Vec<u8>, Delta> {
    let parents = event["parents"].as_array().unwrap();
    let mut args = vec![
        "diff-tree",
        "--no-commit-id",
        "--raw",
        "--no-abbrev",
        "-r",
        "-z",
        "--no-renames",
    ];
    if let Some(parent) = parents.first() {
        args.push(parent.as_str().unwrap());
    } else {
        args.push("--root");
    }
    args.push(event["id"].as_str().unwrap());
    let raw = git(path, &args);
    let mut fields = raw
        .split(|&byte| byte == 0)
        .filter(|field| !field.is_empty());
    let mut expected = BTreeMap::new();
    while let Some(header) = fields.next() {
        let path = fields.next().expect("raw record has a path").to_vec();
        let parts: Vec<_> = std::str::from_utf8(header)
            .unwrap()
            .trim_start_matches(':')
            .split(' ')
            .collect();
        expected.insert(
            path,
            (
                parts[0].to_owned(),
                parts[1].to_owned(),
                parts[2].to_owned(),
                parts[3].to_owned(),
            ),
        );
    }
    expected
}

fn imported_deltas(event: &Value) -> BTreeMap<Vec<u8>, Delta> {
    let state = |entry: &Value| {
        if entry.is_null() {
            ("000000".to_owned(), "0".repeat(40))
        } else {
            (
                format!("{:06o}", entry["mode"].as_u64().unwrap()),
                entry["id"].as_str().unwrap().to_owned(),
            )
        }
    };
    let mut actual = BTreeMap::new();
    for change in event["changes"].as_array().unwrap() {
        let path = bytes(&change["path"]);
        let (old_mode, old_id) = state(&change["before"]);
        let (new_mode, new_id) = state(&change["after"]);
        if change["kind"] == "rename_candidate" {
            actual.insert(
                bytes(&change["source_path"]),
                (old_mode, "000000".to_owned(), old_id, "0".repeat(40)),
            );
            actual.insert(
                path,
                ("000000".to_owned(), new_mode, "0".repeat(40), new_id),
            );
        } else {
            actual.insert(path, (old_mode, new_mode, old_id, new_id));
        }
    }
    actual
}

fn assert_history(path: &Path, commits: &[Value]) {
    let expected_ids: BTreeSet<_> =
        String::from_utf8(git(path, &["rev-list", "inspected"]))
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect();
    let mut seen = BTreeSet::new();
    let mut all_paths = BTreeSet::new();
    for event in commits {
        let id = event["id"].as_str().unwrap();
        for parent in event["parents"].as_array().unwrap() {
            assert!(seen.contains(parent.as_str().unwrap()));
        }
        seen.insert(id.to_owned());
        assert_eq!(bytes(&event["author"]["name"]), b"Raw Author");
        assert_eq!(bytes(&event["author"]["email"]), b"author@example.invalid");
        assert_eq!(bytes(&event["committer"]["name"]), b"Other Committer");
        assert_eq!(event["author"]["seconds"], 1_700_000_000_i64);
        assert_eq!(event["committer"]["seconds"], 1_700_000_060_i64);
        assert_eq!(event["author"]["offset_seconds"], 19800);
        assert_eq!(event["committer"]["offset_seconds"], -25200);
        let actual = imported_deltas(event);
        all_paths.extend(actual.keys().cloned());
        assert_eq!(actual, native_deltas(path, event), "tree changes for {id}");
    }
    assert_eq!(seen, expected_ids);
    assert!(all_paths.contains(b"space name.txt".as_slice()));
    #[cfg(all(unix, not(target_os = "macos")))]
    assert!(all_paths.contains(b"nonutf8-\xff\n".as_slice()));
    let changes: Vec<_> = commits
        .iter()
        .flat_map(|event| event["changes"].as_array().unwrap())
        .collect();
    assert!(
        changes
            .iter()
            .any(|change| change["kind"] == "rename_candidate"
                && bytes(&change["source_path"]) == b"text.txt")
    );
    assert!(
        changes
            .iter()
            .any(|change| change["diff"]["kind"] == "binary")
    );
    assert!(changes.iter().all(|change| {
        bytes(&change["path"]) != b"moved/empty" || change["kind"] == "addition"
    }));
    assert!(
        changes
            .iter()
            .any(|change| bytes(&change["path"]) == b"renamed.txt"
                && change["diff"]["insertions"] == 2
                && change["diff"]["removals"] == 1)
    );
    assert!(
        commits
            .iter()
            .any(|event| event["changes"].as_array().unwrap().is_empty())
    );
    assert!(
        commits
            .iter()
            .any(|event| event["parents"].as_array().unwrap().len() == 2)
    );
}

#[test]
fn history_matches_git_and_preserves_repository() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path();
    fixture(path);
    let first = inspect(path, "inspected");
    commit(path, "later excluded");
    let status_before = git(path, &["status", "--porcelain=v1", "-z"]);
    let refs_before = git(path, &["show-ref"]);
    let index_before = fs::read(path.join(".git/index")).unwrap();
    let second = inspect(path, "inspected");
    assert_eq!(first, second);
    assert_eq!(
        status_before,
        git(path, &["status", "--porcelain=v1", "-z"])
    );
    assert_eq!(refs_before, git(path, &["show-ref"]));
    assert_eq!(index_before, fs::read(path.join(".git/index")).unwrap());
    let commits = first["commits"].as_array().unwrap();
    assert_eq!(commits.len(), 6);
    assert_history(path, commits);
    let root = inspect(path, "root");
    assert_eq!(root["commits"].as_array().unwrap().len(), 1);
    let bare = path.join("bare.git");
    git(path, &["clone", "--bare", ".", bare.to_str().unwrap()]);
    assert_eq!(second, inspect(&bare, "inspected"));
}

#[test]
fn invalid_repository_and_revision_fail_without_json() {
    let scratch = tempfile::tempdir().unwrap();
    for revision in ["HEAD", "does-not-exist"] {
        let output = Command::new(env!("CARGO_BIN_EXE_annealer"))
            .arg("inspect")
            .arg(scratch.path())
            .args(["--revision", revision])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert_eq!(output.stdout, [] as [u8; 0]);
        git(scratch.path(), &["init", "--initial-branch=main"]);
        commit(scratch.path(), "root");
    }
}

#[test]
fn raw_signature_and_message_bytes_survive_import() {
    use std::{io::Write, process::Stdio};
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path();
    git(path, &["init", "--initial-branch=main"]);
    let tree = String::from_utf8(git(path, &["write-tree"])).unwrap();
    let mut data = format!("tree {}\n", tree.trim()).into_bytes();
    data.extend_from_slice(b"author  A\xff  < email\xfe > 1700000000 +0530\ncommitter C\xfd <c\xfc> 1700000060 -0700\n\nraw\xfb\n");
    let mut child = Command::new("git")
        .arg("-C")
        .arg(path)
        .args(["hash-object", "-t", "commit", "-w", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&data).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let id = String::from_utf8(output.stdout).unwrap();
    let value = inspect(path, id.trim());
    let event = &value["commits"][0];
    assert_eq!(bytes(&event["author"]["name"]), b" A\xff ");
    assert_eq!(bytes(&event["author"]["email"]), b" email\xfe ");
    assert_eq!(
        bytes(&event["author"]["raw_signature"]),
        b" A\xff  < email\xfe > 1700000000 +0530"
    );
    assert_eq!(bytes(&event["committer"]["name"]), b"C\xfd");
    assert_eq!(bytes(&event["message"]), b"raw\xfb\n");
    assert_eq!(event["author"]["seconds"], 1_700_000_000_i64);
}

#[test]
fn attributes_and_commands_cannot_change_raw_blob_results() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path();
    fixture(path);
    let baseline = inspect(path, "inspected");
    fs::write(path.join(".gitattributes"), b"*.txt -diff\n").unwrap();
    git(path, &["add", ".gitattributes"]);
    assert_eq!(baseline, inspect(path, "inspected"));
    fs::write(path.join(".gitattributes"), b"*.txt diff=probe\n").unwrap();
    git(path, &["add", ".gitattributes"]);
    git(path, &["config", "diff.probe.textconv", "git --version"]);
    git(
        path,
        &["config", "diff.external", "command-that-does-not-exist"],
    );
    let converted = git(path, &["show", "--textconv", "inspected:renamed.txt"]);
    assert!(converted.starts_with(b"git version"));
    assert_eq!(baseline, inspect(path, "inspected"));
}

#[test]
fn shallow_history_fails_instead_of_inventing_a_root() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path();
    fixture(path);
    let shallow = path.join("shallow");
    git(
        path,
        &[
            "clone",
            "--depth=1",
            "--no-local",
            ".",
            shallow.to_str().unwrap(),
        ],
    );
    let output = Command::new(env!("CARGO_BIN_EXE_annealer"))
        .arg("inspect")
        .arg(&shallow)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(output.stdout, [] as [u8; 0]);
    assert!(String::from_utf8_lossy(&output.stderr).contains("complete local history"));
}

#[test]
fn modes_symlinks_and_gitlinks_retain_object_identity() {
    let scratch = tempfile::tempdir().unwrap();
    let path = scratch.path();
    git(path, &["init", "--initial-branch=main"]);
    fs::write(path.join("content"), b"stored target\n").unwrap();
    commit(path, "root");
    let root = String::from_utf8(git(path, &["rev-parse", "HEAD"])).unwrap();
    let blob = String::from_utf8(git(path, &["rev-parse", "HEAD:content"])).unwrap();
    git(
        path,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("120000,{},link", blob.trim()),
        ],
    );
    git(
        path,
        &[
            "update-index",
            "--add",
            "--cacheinfo",
            &format!("160000,{},submodule", root.trim()),
        ],
    );
    git(path, &["update-index", "--chmod=+x", "content"]);
    git(
        path,
        &["-c", "commit.gpgsign=false", "commit", "-m", "modes"],
    );
    let history = inspect(path, "HEAD");
    let event = &history["commits"][1];
    assert_eq!(imported_deltas(event), native_deltas(path, event));
    let changes = event["changes"].as_array().unwrap();
    let link = changes
        .iter()
        .find(|change| bytes(&change["path"]) == b"link")
        .unwrap();
    assert_eq!(link["after"]["mode"], 0o120_000);
    assert_eq!(link["diff"]["kind"], "text");
    let submodule = changes
        .iter()
        .find(|change| bytes(&change["path"]) == b"submodule")
        .unwrap();
    assert_eq!(submodule["after"]["id"], root.trim());
    assert_eq!(submodule["diff"]["kind"], "gitlink");
    let executable = changes
        .iter()
        .find(|change| bytes(&change["path"]) == b"content")
        .unwrap();
    assert_eq!(executable["after"]["mode"], 0o100_755);
    assert_eq!(executable["diff"]["insertions"], 0);
    assert_eq!(executable["diff"]["removals"], 0);
}
