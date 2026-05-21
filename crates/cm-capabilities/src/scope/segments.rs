use cm_core::ScopePath;

#[derive(Debug, Default)]
pub(super) struct ScopeSegments {
    pub(super) projects: Vec<String>,
    pub(super) repo: Option<String>,
}

impl ScopeSegments {
    pub(super) fn leaf_project(&self) -> Option<&str> {
        self.projects.last().map(String::as_str)
    }
}

pub(super) fn scope_segments(path: &ScopePath) -> ScopeSegments {
    let mut segments = ScopeSegments::default();
    for segment in path.as_str().split('/').skip(1) {
        let Some((kind, id)) = segment.split_once(':') else {
            continue;
        };
        match kind {
            "project" => segments.projects.push(id.to_owned()),
            "repo" => segments.repo = Some(id.to_owned()),
            _ => {}
        }
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_segments_collects_projects_outer_to_inner() {
        let path = ScopePath::parse("global/project:helioy/project:agents/repo:nancyr").unwrap();

        let segments = scope_segments(&path);

        assert_eq!(segments.projects, vec!["helioy", "agents"]);
        assert_eq!(segments.leaf_project(), Some("agents"));
        assert_eq!(segments.repo.as_deref(), Some("nancyr"));
    }
}
