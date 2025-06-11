use std::path::PathBuf;

/// Finds the common base path between two PathBuf instances and calculates
/// the relative paths from the base to each original path.
///
/// # Arguments
/// * `source` - The source PathBuf (must be canonical)
/// * `target` - The target PathBuf (must be canonical)
///
/// # Returns
/// A tuple containing:
/// * `base` - The common base path
/// * `rel_source` - The relative path from base to source
/// * `rel_target` - The relative path from base to target
pub(crate) fn find_common_base(source: &PathBuf, target: &PathBuf) -> (PathBuf, PathBuf, PathBuf) {
    let source_components: Vec<_> = source.components().collect();
    let target_components: Vec<_> = target.components().collect();

    // Find the length of the common prefix by comparing components
    let common_len = source_components
        .iter()
        .zip(target_components.iter())
        .take_while(|(a, b)| a == b)
        .count();

    // Build the base path from the common components
    let base: PathBuf = source_components[..common_len].iter().collect();

    // Build the relative paths from the remaining components
    let rel_source: PathBuf = source_components[common_len..].iter().collect();
    let rel_target: PathBuf = target_components[common_len..].iter().collect();

    (base, rel_source, rel_target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_common_parent_directory() {
        let source = PathBuf::from("/home/user/documents/file1.txt");
        let target = PathBuf::from("/home/user/pictures/photo.jpg");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/home/user"));
        assert_eq!(rel_source, PathBuf::from("documents/file1.txt"));
        assert_eq!(rel_target, PathBuf::from("pictures/photo.jpg"));
    }

    #[test]
    fn test_one_is_prefix_of_other() {
        let source = PathBuf::from("/home/user");
        let target = PathBuf::from("/home/user/documents/file.txt");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/home/user"));
        assert_eq!(rel_source, PathBuf::from(""));
        assert_eq!(rel_target, PathBuf::from("documents/file.txt"));
    }

    #[test]
    fn test_target_is_prefix_of_source() {
        let source = PathBuf::from("/home/user/documents/file.txt");
        let target = PathBuf::from("/home/user");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/home/user"));
        assert_eq!(rel_source, PathBuf::from("documents/file.txt"));
        assert_eq!(rel_target, PathBuf::from(""));
    }

    #[test]
    fn test_identical_paths() {
        let source = PathBuf::from("/home/user/file.txt");
        let target = PathBuf::from("/home/user/file.txt");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/home/user/file.txt"));
        assert_eq!(rel_source, PathBuf::from(""));
        assert_eq!(rel_target, PathBuf::from(""));
    }

    #[test]
    fn test_divergent_paths() {
        let source = PathBuf::from("/home/user1/documents/file.txt");
        let target = PathBuf::from("/home/user2/pictures/photo.jpg");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/home"));
        assert_eq!(rel_source, PathBuf::from("user1/documents/file.txt"));
        assert_eq!(rel_target, PathBuf::from("user2/pictures/photo.jpg"));
    }

    #[test]
    fn test_completely_different_paths() {
        let source = PathBuf::from("/home/user/file.txt");
        let target = PathBuf::from("/var/log/system.log");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/"));
        assert_eq!(rel_source, PathBuf::from("home/user/file.txt"));
        assert_eq!(rel_target, PathBuf::from("var/log/system.log"));
    }

    #[test]
    fn test_root_paths() {
        let source = PathBuf::from("/");
        let target = PathBuf::from("/home");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/"));
        assert_eq!(rel_source, PathBuf::from(""));
        assert_eq!(rel_target, PathBuf::from("home"));
    }

    #[test]
    fn test_deep_nesting() {
        let source = PathBuf::from("/a/b/c/d/e/f/file1.txt");
        let target = PathBuf::from("/a/b/c/x/y/z/file2.txt");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from("/a/b/c"));
        assert_eq!(rel_source, PathBuf::from("d/e/f/file1.txt"));
        assert_eq!(rel_target, PathBuf::from("x/y/z/file2.txt"));
    }

    #[test]
    #[cfg(windows)]
    fn test_different_drives_windows() {
        let source = PathBuf::from(r"C:\Users\user\file.txt");
        let target = PathBuf::from(r"D:\Data\other.txt");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        // Different drives should have no common base
        assert_eq!(base, PathBuf::from(""));
        assert_eq!(rel_source, PathBuf::from(r"C:\Users\user\file.txt"));
        assert_eq!(rel_target, PathBuf::from(r"D:\Data\other.txt"));
    }

    #[test]
    #[cfg(windows)]
    fn test_same_drive_windows() {
        let source = PathBuf::from(r"C:\Users\user1\Documents\file.txt");
        let target = PathBuf::from(r"C:\Users\user2\Pictures\photo.jpg");

        let (base, rel_source, rel_target) = find_common_base(&source, &target);

        assert_eq!(base, PathBuf::from(r"C:\Users"));
        assert_eq!(rel_source, PathBuf::from(r"user1\Documents\file.txt"));
        assert_eq!(rel_target, PathBuf::from(r"user2\Pictures\photo.jpg"));
    }
}
