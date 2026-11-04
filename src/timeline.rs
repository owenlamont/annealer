use crate::history::{BlobDiff, ChangeKind, History};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum Kind {
    Added,
    Deleted,
    Modified,
    Moved { from: String },
}

#[derive(Clone, Debug)]
pub(super) struct FileChange {
    pub path: String,
    pub kind: Kind,
    pub insertions: u32,
    pub removals: u32,
}

#[derive(Clone, Debug)]
pub(super) struct Event {
    pub author: String,
    pub seconds: i64,
    pub changes: Vec<FileChange>,
}

pub(super) fn from_history(history: &History) -> Vec<Event> {
    history
        .commits
        .iter()
        .filter(|commit| !commit.changes.is_empty())
        .map(|commit| Event {
            author: String::from_utf8_lossy(&commit.author.name).into_owned(),
            seconds: commit.author.seconds,
            changes: commit
                .changes
                .iter()
                .map(|change| {
                    let (insertions, removals) = match change.diff {
                        BlobDiff::Text {
                            insertions,
                            removals,
                            ..
                        } => (insertions, removals),
                        BlobDiff::Binary | BlobDiff::Gitlink => (1, 1),
                    };
                    let kind = match (&change.kind, &change.source_path) {
                        (ChangeKind::Addition, _) => Kind::Added,
                        (ChangeKind::Deletion, _) => Kind::Deleted,
                        (ChangeKind::RenameCandidate, Some(from)) => Kind::Moved {
                            from: String::from_utf8_lossy(from).into_owned(),
                        },
                        (ChangeKind::Modification | ChangeKind::RenameCandidate, _) => {
                            Kind::Modified
                        }
                    };
                    FileChange {
                        path: String::from_utf8_lossy(&change.path).into_owned(),
                        kind,
                        insertions,
                        removals,
                    }
                })
                .collect(),
        })
        .collect()
}

/// A small scripted history exercising growth, moves, deletion and a crowded commit.
pub(super) fn synthetic() -> Vec<Event> {
    let mut events = Vec::new();
    let mut push = |author: &str, changes: Vec<FileChange>| {
        let seconds = 1_700_000_000 + 86_400 * i64::try_from(events.len()).unwrap_or(0);
        events.push(Event {
            author: author.to_owned(),
            seconds,
            changes,
        });
    };
    let add = |path: &str, lines| change(path, Kind::Added, lines, 0);
    let edit = |path: &str, insertions, removals| {
        change(path, Kind::Modified, insertions, removals)
    };
    push(
        "Ada",
        vec![
            add("Cargo.toml", 20),
            add("README.md", 40),
            add("src/main.rs", 60),
        ],
    );
    push(
        "Ada",
        vec![add("src/model.rs", 110), edit("src/main.rs", 8, 2)],
    );
    push(
        "Grace",
        vec![add(".github/workflows/ci.yml", 45), add("tests/cli.rs", 40)],
    );
    push(
        "Ada",
        vec![
            add("src/scene.rs", 300),
            add("src/layout.rs", 80),
            edit("src/main.rs", 4, 1),
        ],
    );
    push(
        "Grace",
        vec![edit("README.md", 12, 30), add("docs/guide.md", 70)],
    );
    push(
        "Ada",
        vec![
            add("src/export.rs", 150),
            add("src/old.rs", 30),
            edit("Cargo.toml", 3, 0),
        ],
    );
    push(
        "Linus",
        vec![
            edit("src/scene.rs", 40, 25),
            edit("src/model.rs", 9, 9),
            add("tests/render.rs", 85),
        ],
    );
    let crowd: Vec<_> = (0..64)
        .map(|index| add(&format!("tests/fixtures/case_{index:02}.yaml"), 12))
        .chain([edit("tests/cli.rs", 30, 2)])
        .collect();
    push("Grace", crowd);
    push(
        "Ada",
        vec![
            change(
                "src/scene/layout.rs",
                Kind::Moved {
                    from: "src/layout.rs".to_owned(),
                },
                10,
                4,
            ),
            change("src/old.rs", Kind::Deleted, 0, 30),
            add("src/scene/labels.rs", 55),
        ],
    );
    push(
        "Linus",
        vec![
            edit("src/scene.rs", 5, 60),
            edit(".github/workflows/ci.yml", 6, 2),
        ],
    );
    let sweep: Vec<_> = (0..64)
        .step_by(2)
        .map(|index| edit(&format!("tests/fixtures/case_{index:02}.yaml"), 1, 1))
        .collect();
    push("Grace", sweep);
    push(
        "Ada",
        vec![
            edit("README.md", 20, 4),
            change("docs/guide.md", Kind::Deleted, 0, 70),
            add("docs/user/guide.md", 90),
        ],
    );
    events
}

fn change(path: &str, kind: Kind, insertions: u32, removals: u32) -> FileChange {
    FileChange {
        path: path.to_owned(),
        kind,
        insertions,
        removals,
    }
}
