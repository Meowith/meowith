use crate::dto::config::FsLimitConfiguration;
use lazy_static::lazy_static;
use regex::Regex;
use std::path::Path;

/// Split the given file path into a format friendly to the database
///
/// ```
/// use data::pathlib::split_path;
/// assert_eq!(split_path("/a/path/to/a.txt"),  (Some("a/path/to".to_string()), "a.txt".to_string()));
/// assert_eq!(split_path("a/path/to/a.txt"),   (Some("a/path/to".to_string()), "a.txt".to_string()));
/// assert_eq!(split_path("a\\path\\to/a.txt"), (Some("a/path/to".to_string()), "a.txt".to_string()));
/// assert_eq!(split_path("a.txt"),             (None, "a.txt".to_string()));
/// assert_eq!(split_path("/a.txt"),             (None, "a.txt".to_string()));
/// ```
pub fn split_path(file_path: &str) -> (Option<String>, String) {
    let path = Path::new(file_path);
    let mut parent = path.parent().map(|path| normalize(&path.to_string_lossy()));
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

pub fn join_parent_name(parent: &str, name: &String) -> String {
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
    PATH_REGEX
        .replace_all(path, "/")
        .trim_matches('/')
        .to_string()
}

/// Prepare the provided path to be used by meowith.
/// Normalize the path and validate it.
/// If all is good return Some(path), else None.
///
/// # Examples
///
/// ```
/// use data::dto::config::FsLimitConfiguration;
/// use data::pathlib::prepare_path;
///
/// let config = FsLimitConfiguration {
///     max_path_length: 50,
///     max_directory_depth: 3,
/// };
///
/// assert_eq!(prepare_path("/valid/path", &config), Some("valid/path".to_string()));
/// assert_eq!(prepare_path("too/deep/path/structure", &config), None);
/// assert_eq!(prepare_path(&"a".repeat(51), &config), None);
/// ```
pub fn prepare_path(path: &str, fs_limit_configuration: &FsLimitConfiguration) -> Option<String> {
    if path.len() as u32 > fs_limit_configuration.auto_reject_path_length() {
        return None;
    }
    let normalized = normalize(path);
    let nest_level = normalized.chars().filter(|&c| c == '/').count() as u32 + 1;

    if nest_level > fs_limit_configuration.max_directory_depth
        || normalized.len() as u32 > fs_limit_configuration.max_path_length
    {
        return None;
    }

    Some(normalized)
}
