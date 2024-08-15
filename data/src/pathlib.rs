use std::path::Path;
use lazy_static::lazy_static;
use regex::Regex;

/// Split the given file path into a format friendly to the database
///
/// ```
/// use data::pathlib::split_path;
/// assert_eq!(split_path("/a/path/to/a.txt"),  (Some("a/path/to".to_string()), "a.txt".to_string()));
/// assert_eq!(split_path("a/path/to/a.txt"),   (Some("a/path/to".to_string()), "a.txt".to_string()));
/// assert_eq!(split_path("a\\path\\to/a.txt"), (Some("a/path/to".to_string()), "a.txt".to_string()));
/// assert_eq!(split_path("a.txt"),             (None, "a.txt".to_string()));
/// ```
pub fn split_path(file_path: &str) -> (Option<String>, String) {
    let path = Path::new(file_path);
    let mut parent = path
        .parent()
        .map(|path| normalize(&path.to_string_lossy()));
    if parent.as_ref().is_some_and(|s| s.is_empty()) {
        parent = None
    }

    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    (parent, normalize(&file_name))
}

pub fn join_parent_name(parent: &String, name: &String) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{parent}/{name}")
    }
}

lazy_static! {
    static ref PATH_REGEX: Regex = Regex::new(r"[\\/]+").unwrap();
}

/// Normalize the path into a format friendly to meowith.
/// Replace every \ into / and coalesce sequences of '///' into singular /.
/// Lastly remove any leading and trailing /
///
/// ```
/// use data::pathlib::normalize;
/// assert_eq!(normalize("/a/path/to/a.txt"),       "a/path/to/a.txt".to_string());
/// assert_eq!(normalize("a/path/to/a.txt"),        "a/path/to/a.txt".to_string());
/// assert_eq!(normalize("a\\path\\/\\\\to/a.txt"), "a/path/to/a.txt".to_string());
/// assert_eq!(normalize("a.txt"),                  "a.txt".to_string());
/// ```
pub fn normalize(path: &str) -> String {
    PATH_REGEX.replace_all(path, "/").trim_matches('/').to_string()
}
