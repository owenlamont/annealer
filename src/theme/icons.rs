use std::sync::LazyLock;

use super::FileGlyph;

#[derive(serde::Deserialize)]
struct Association {
    name: String,
    filenames: Vec<String>,
    extensions: Vec<String>,
}

const SOURCES: &[(&str, &str)] = &[
    ("file", include_str!("../../assets/material/svg/file.svg")),
    ("html", include_str!("../../assets/material/svg/html.svg")),
    (
        "markdown",
        include_str!("../../assets/material/svg/markdown.svg"),
    ),
    ("css", include_str!("../../assets/material/svg/css.svg")),
    ("json", include_str!("../../assets/material/svg/json.svg")),
    ("yaml", include_str!("../../assets/material/svg/yaml.svg")),
    ("toml", include_str!("../../assets/material/svg/toml.svg")),
    (
        "javascript",
        include_str!("../../assets/material/svg/javascript.svg"),
    ),
    ("react", include_str!("../../assets/material/svg/react.svg")),
    (
        "react_ts",
        include_str!("../../assets/material/svg/react_ts.svg"),
    ),
    (
        "typescript",
        include_str!("../../assets/material/svg/typescript.svg"),
    ),
    ("c", include_str!("../../assets/material/svg/c.svg")),
    ("h", include_str!("../../assets/material/svg/h.svg")),
    ("cpp", include_str!("../../assets/material/svg/cpp.svg")),
    ("hpp", include_str!("../../assets/material/svg/hpp.svg")),
    ("go", include_str!("../../assets/material/svg/go.svg")),
    (
        "go-mod",
        include_str!("../../assets/material/svg/go-mod.svg"),
    ),
    (
        "python",
        include_str!("../../assets/material/svg/python.svg"),
    ),
    (
        "python-misc",
        include_str!("../../assets/material/svg/python-misc.svg"),
    ),
    ("ruff", include_str!("../../assets/material/svg/ruff.svg")),
    ("uv", include_str!("../../assets/material/svg/uv.svg")),
    (
        "console",
        include_str!("../../assets/material/svg/console.svg"),
    ),
    (
        "powershell",
        include_str!("../../assets/material/svg/powershell.svg"),
    ),
    (
        "license",
        include_str!("../../assets/material/svg/license.svg"),
    ),
    (
        "docker",
        include_str!("../../assets/material/svg/docker.svg"),
    ),
    ("rust", include_str!("../../assets/material/svg/rust.svg")),
    ("git", include_str!("../../assets/material/svg/git.svg")),
    ("lock", include_str!("../../assets/material/svg/lock.svg")),
    (
        "nodejs",
        include_str!("../../assets/material/svg/nodejs.svg"),
    ),
    ("npm", include_str!("../../assets/material/svg/npm.svg")),
    ("yarn", include_str!("../../assets/material/svg/yarn.svg")),
    (
        "contributing",
        include_str!("../../assets/material/svg/contributing.svg"),
    ),
    (
        "readme",
        include_str!("../../assets/material/svg/readme.svg"),
    ),
    (
        "changelog",
        include_str!("../../assets/material/svg/changelog.svg"),
    ),
    (
        "editorconfig",
        include_str!("../../assets/material/svg/editorconfig.svg"),
    ),
    ("pnpm", include_str!("../../assets/material/svg/pnpm.svg")),
];

static GLYPHS: LazyLock<Vec<FileGlyph>> = LazyLock::new(|| {
    let associations: Vec<Association> =
        serde_json::from_str(include_str!("../../assets/material/mapping.json"))
            .expect("vendored icon mapping is valid JSON");
    assert_eq!(
        associations.len(),
        SOURCES.len(),
        "mapping and SVG registry agree"
    );
    associations
        .into_iter()
        .map(|association| {
            let (_, svg) = SOURCES
                .iter()
                .find(|(name, _)| *name == association.name)
                .expect("each mapped icon has an embedded SVG");
            FileGlyph {
                svg,
                filenames: association.filenames,
                extensions: association.extensions,
            }
        })
        .collect()
});

pub(super) fn material() -> &'static [FileGlyph] {
    &GLYPHS
}
