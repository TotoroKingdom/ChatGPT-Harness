use globset::{GlobBuilder, GlobMatcher};

/// Include/exclude filters use the existing Workspace-relative path semantics.
/// Exclude always wins over include and over an explicit ignore override.
#[derive(Clone, Debug, Default)]
pub(crate) struct PathFilters {
    include: Vec<GlobMatcher>,
    exclude: Vec<GlobMatcher>,
    prune_prefixes: Vec<String>,
}

impl PathFilters {
    pub(crate) fn compile(include: &[String], exclude: &[String]) -> Result<Self, globset::Error> {
        Ok(Self {
            include: compile_all(include)?,
            exclude: compile_all(exclude)?,
            prune_prefixes: exclude
                .iter()
                .filter_map(|pattern| prune_prefix(pattern))
                .collect(),
        })
    }

    pub(crate) fn allows(&self, logical_path: &str) -> bool {
        let included =
            self.include.is_empty() || self.include.iter().any(|glob| glob.is_match(logical_path));
        included && !self.exclude.iter().any(|glob| glob.is_match(logical_path))
    }

    pub(crate) fn includes(&self) -> bool {
        !self.include.is_empty()
    }

    /// A directory may be pruned only when an exclude rule provably covers
    /// every descendant. Broad patterns are still applied to files.
    pub(crate) fn prunes_directory(&self, logical_path: &str) -> bool {
        self.prune_prefixes.iter().any(|prefix| {
            logical_path == prefix
                || logical_path
                    .strip_prefix(prefix)
                    .is_some_and(|suffix| suffix.starts_with('/'))
        })
    }
}

fn compile_all(patterns: &[String]) -> Result<Vec<GlobMatcher>, globset::Error> {
    patterns
        .iter()
        .map(|pattern| {
            GlobBuilder::new(pattern)
                .literal_separator(true)
                .build()
                .map(|glob| glob.compile_matcher())
        })
        .collect()
}

/// `build/**` means the whole subtree is excluded, so `build` can be pruned.
fn prune_prefix(pattern: &str) -> Option<String> {
    let prefix = pattern.strip_suffix("/**")?;
    let prefix = prefix.trim_end_matches('/');
    if prefix.is_empty() || prefix.split('/').any(|segment| has_wildcard(segment)) {
        return None;
    }
    Some(prefix.to_owned())
}

pub(crate) fn has_wildcard(segment: &str) -> bool {
    segment
        .bytes()
        .any(|byte| matches!(byte, b'*' | b'?' | b'[' | b'{'))
}

/// Literal leading directory segments before the first wildcard segment.
pub(crate) fn fixed_directory_prefix(pattern: &str) -> Option<String> {
    let segments = pattern.split('/').collect::<Vec<_>>();
    let mut prefix = Vec::new();
    for segment in &segments {
        if has_wildcard(segment) {
            break;
        }
        prefix.push(*segment);
    }
    if prefix.is_empty() || prefix.len() == segments.len() {
        return None;
    }
    let prefix = prefix.join("/");
    if prefix.is_empty() || prefix == "." {
        None
    } else {
        Some(prefix)
    }
}

/// Explicit prefixes and scopes that must still be traversed when ignored.
pub(crate) fn ignore_overrides(prefix: &str) -> Vec<String> {
    if prefix.is_empty() || prefix == "." || prefix.split('/').any(has_wildcard) {
        return Vec::new();
    }
    // OverrideBuilder uses the inverse of gitignore syntax: a pattern without
    // `!` is a whitelist and therefore overrides an ignore rule. Whitelist the
    // directory itself so an ignored parent directory is still descended into,
    // then every descendant so nested Git ignore rules do not hide results.
    vec![prefix.to_owned(), format!("{prefix}/**")]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_prefixes_are_literal_only_and_never_cover_broad_patterns() {
        assert_eq!(
            fixed_directory_prefix("target/**/*.o").as_deref(),
            Some("target")
        );
        assert_eq!(
            fixed_directory_prefix("src/nested/*.rs").as_deref(),
            Some("src/nested")
        );
        assert_eq!(fixed_directory_prefix("**/*.rs"), None);
        assert_eq!(fixed_directory_prefix("*.rs"), None);
        assert_eq!(fixed_directory_prefix("src/*.rs"), Some("src".into()));
    }

    #[test]
    fn exclude_wins_and_subtree_pruning_requires_explicit_recursive_rule() {
        let filters = PathFilters::compile(
            &["docs/**".into()],
            &["docs/skip.md".into(), "target/**".into(), "vendor".into()],
        )
        .unwrap();
        assert!(filters.allows("docs/keep.md"));
        assert!(!filters.allows("docs/skip.md"));
        assert!(!filters.allows("src/main.rs"));
        assert!(filters.prunes_directory("target"));
        assert!(filters.prunes_directory("target/debug"));
        assert!(!filters.prunes_directory("vendor"));
        assert!(!filters.prunes_directory("vendor/deep"));
    }
}
