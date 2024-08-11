use std::path::Path;

/// Split the given file path into a format friendly to the database
///
/// ```
/// use commons::pathlib::split_path;
/// assert_eq!(split_path("/a/path/to/a.txt"),  ("a/path/to".to_string(), "a.txt".to_string()));
/// assert_eq!(split_path("a/path/to/a.txt"),   ("a/path/to".to_string(), "a.txt".to_string()));
/// assert_eq!(split_path("a\\path\\to/a.txt"), ("a/path/to".to_string(), "a.txt".to_string()));
/// assert_eq!(split_path("a.txt"),             ("".to_string(), "a.txt".to_string()));
/// ```
pub fn split_path(file_path: &str) -> (String, String) {
    let normalized_path = file_path.replace('\\', "/");

    let path = Path::new(&normalized_path);
    let parent = path.parent().unwrap_or(Path::new("")).to_path_buf();
    let file_name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();

    let mut parent_str = parent.to_string_lossy().into_owned();

    // Strip leading and trailing slashes
    parent_str = parent_str.trim_matches('/').to_string();

    (parent_str, file_name)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_split_path() {
        let cases = vec![
            ("/a/path/to/a.txt", "a/path/to", "a.txt"),
            ("a/path/to/a.txt", "a/path/to", "a.txt"),
            ("a\\path\\to/a.txt", "a/path/to", "a.txt"),
            ("a.txt", "", "a.txt"),
        ];

        for case in cases {
            assert_eq!(
                crate::pathlib::split_path(&case.0.to_string()),
                (case.1.to_string(), case.2.to_string())
            );
        }
    }
}
