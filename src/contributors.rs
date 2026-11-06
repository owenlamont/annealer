use std::{
    collections::HashMap,
    fs, io,
    path::{Path, PathBuf},
};

use serde::Deserialize;

use crate::timeline::Event;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Key {
    Email(Vec<u8>),
    Name(Vec<u8>),
    Config(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Contributor {
    pub key: Key,
    pub name: String,
}

#[derive(Debug, thiserror::Error)]
pub(super) enum Error {
    #[error("read contributors file {}", path.display())]
    Read { path: PathBuf, source: io::Error },
    #[error("parse contributors file {}", path.display())]
    Parse {
        path: PathBuf,
        source: toml::de::Error,
    },
    #[error("contributors file {} lists email {email:?} more than once", path.display())]
    Duplicate { path: PathBuf, email: String },
}

#[derive(Debug, Default)]
pub(super) struct Mappings {
    emails: HashMap<Vec<u8>, Contributor>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Config {
    #[serde(default)]
    contributor: Vec<Entry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Entry {
    name: String,
    emails: Vec<String>,
}

pub(super) fn load(explicit: Option<&Path>) -> Result<Mappings, Error> {
    let default =
        dirs::config_dir().map(|root| root.join("codestrand/contributors.toml"));
    load_path(explicit.or(default.as_deref()), explicit.is_some())
}

fn load_path(path: Option<&Path>, required: bool) -> Result<Mappings, Error> {
    let Some(path) = path else {
        return Ok(Mappings::default());
    };
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(source) if !required && source.kind() == io::ErrorKind::NotFound => {
            return Ok(Mappings::default());
        }
        Err(source) => {
            return Err(Error::Read {
                path: path.to_owned(),
                source,
            });
        }
    };
    Mappings::parse(&text, path)
}

impl Mappings {
    fn parse(text: &str, path: &Path) -> Result<Self, Error> {
        let config: Config = toml::from_str(text).map_err(|source| Error::Parse {
            path: path.to_owned(),
            source,
        })?;
        let mut emails = HashMap::new();
        for (index, entry) in config.contributor.into_iter().enumerate() {
            for email in entry.emails {
                let key = email.as_bytes().to_ascii_lowercase();
                let contributor = Contributor {
                    key: Key::Config(index),
                    name: entry.name.clone(),
                };
                if emails.insert(key, contributor).is_some() {
                    return Err(Error::Duplicate {
                        path: path.to_owned(),
                        email,
                    });
                }
            }
        }
        Ok(Self { emails })
    }

    pub(super) fn apply(&self, events: &mut [Event]) {
        let mut names = HashMap::new();
        for event in events.iter() {
            names.insert(
                unlisted_key(event),
                String::from_utf8_lossy(&event.author.name).into_owned(),
            );
        }
        for event in events {
            let email = event.author.email.to_ascii_lowercase();
            event.contributor =
                Some(self.emails.get(&email).cloned().unwrap_or_else(|| {
                    let key = unlisted_key(event);
                    Contributor {
                        name: names[&key].clone(),
                        key,
                    }
                }));
        }
    }
}

fn unlisted_key(event: &Event) -> Key {
    if event.author.email.is_empty() {
        Key::Name(event.author.name.clone())
    } else {
        Key::Email(event.author.email.to_ascii_lowercase())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        model::{Options, Simulation},
        theme::Theme,
        timeline::{self, Identity},
    };

    fn events(identities: &[(&str, &str)]) -> Vec<Event> {
        identities
            .iter()
            .map(|&(name, email)| Event {
                author: Identity {
                    name: name.as_bytes().to_vec(),
                    email: email.as_bytes().to_vec(),
                },
                contributor: None,
                seconds: 0,
                changes: vec![timeline::FileChange {
                    path: email.to_owned(),
                    kind: timeline::Kind::Modified,
                    insertions: 1,
                    removals: 0,
                }],
            })
            .collect()
    }

    fn names(events: &[Event], mappings: &Mappings) -> Vec<String> {
        let mut sim = Simulation::new(
            events.to_vec(),
            &Options::default(),
            &Theme::neon(),
            mappings,
        );
        sim.advance_to(120, 30);
        sim.avatars()
            .iter()
            .map(|avatar| avatar.name.clone())
            .collect()
    }

    #[test]
    fn two_emails_share_an_avatar_and_colour_identity() {
        let mappings = Mappings::parse(
            r#"
[[contributor]]
name = "Owen Lamont"
emails = ["personal@example.invalid", "work@example.invalid"]
"#,
            Path::new("contributors.toml"),
        )
        .unwrap();
        let mut events = events(&[
            ("Personal", "PERSONAL@example.invalid"),
            ("Work", "work@example.invalid"),
            ("Unknown", "unknown@example.invalid"),
        ]);
        let raw = events.clone();
        mappings.apply(&mut events);
        assert_eq!(events[0].contributor, events[1].contributor);
        assert_eq!(events[0].author, raw[0].author);
        assert_eq!(events[1].author, raw[1].author);
        assert_eq!(names(&events, &mappings), ["Owen Lamont", "Unknown"]);
    }

    #[test]
    fn identical_names_do_not_merge_unknown_emails() {
        let events = events(&[
            ("Same Name", "one@example.invalid"),
            ("Same Name", "two@example.invalid"),
        ]);
        assert_eq!(
            names(&events, &Mappings::default()),
            ["Same Name", "Same Name"]
        );
    }

    #[test]
    fn empty_emails_keep_different_authors_separate() {
        let events = events(&[("Alice", ""), ("Bob", "")]);
        assert_eq!(names(&events, &Mappings::default()), ["Alice", "Bob"]);
    }

    #[test]
    fn unknown_tables_and_fields_report_the_file_and_key() {
        for (text, key) in [
            ("[[contributors]]", "contributors"),
            (
                "[[contributor]]\nname = 'Alice'\nemails = []\nemials = []",
                "emials",
            ),
        ] {
            let error = Mappings::parse(text, Path::new("aliases.toml")).unwrap_err();
            assert!(matches!(error, Error::Parse { .. }));
            assert!(error.to_string().contains("aliases.toml"));
            let cause = std::error::Error::source(&error).unwrap().to_string();
            assert!(cause.contains("unknown field"), "{cause}");
            assert!(cause.contains(key), "{cause}");
        }
    }

    #[test]
    fn unknown_email_keeps_the_last_playback_name() {
        let mut events = events(&[
            ("Old", "same@example.invalid"),
            ("Recent", "SAME@example.invalid"),
            ("Unknown", "other@example.invalid"),
        ]);
        Mappings::default().apply(&mut events);
        assert_eq!(events[0].contributor.as_ref().unwrap().name, "Recent");
        assert_eq!(events[2].contributor.as_ref().unwrap().name, "Unknown");
        assert_eq!(names(&events, &Mappings::default()), ["Recent", "Unknown"]);
    }

    #[test]
    fn duplicate_email_reports_the_file_and_email() {
        let error = Mappings::parse(
            r#"
[[contributor]]
name = "One"
emails = ["same@example.invalid"]
[[contributor]]
name = "Two"
emails = ["SAME@example.invalid"]
"#,
            Path::new("aliases.toml"),
        )
        .unwrap_err();
        assert!(matches!(error, Error::Duplicate { .. }));
        assert!(error.to_string().contains("aliases.toml"));
        assert!(error.to_string().contains("SAME@example.invalid"));
    }

    #[test]
    fn malformed_toml_reports_the_file_and_problem() {
        let error =
            Mappings::parse("[[contributor", Path::new("broken.toml")).unwrap_err();
        assert!(matches!(error, Error::Parse { .. }));
        assert!(error.to_string().contains("broken.toml"));
        assert!(
            std::error::Error::source(&error)
                .unwrap()
                .to_string()
                .contains("expected")
        );
    }

    #[test]
    fn only_missing_default_file_is_optional() {
        let scratch = tempfile::tempdir().unwrap();
        let path = scratch.path().join("absent.toml");
        assert!(load_path(Some(&path), false).unwrap().emails.is_empty());
        let error = load(Some(&path)).unwrap_err();
        assert!(matches!(error, Error::Read { .. }));
        assert!(error.to_string().contains("absent.toml"));
        fs::write(&path, "[[contributor").unwrap();
        assert!(matches!(
            load_path(Some(&path), false),
            Err(Error::Parse { .. })
        ));
    }
}
