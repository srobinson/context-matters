use cm_core::ScopePath;

#[derive(Debug, Default)]
pub(super) struct ScopeSegments {
    pub(super) project: Option<String>,
    pub(super) repo: Option<String>,
}

pub(super) fn scope_segments(path: &ScopePath) -> ScopeSegments {
    let mut segments = ScopeSegments::default();
    for segment in path.as_str().split('/').skip(1) {
        let Some((kind, id)) = segment.split_once(':') else {
            continue;
        };
        match kind {
            "project" => segments.project = Some(id.to_owned()),
            "repo" => segments.repo = Some(id.to_owned()),
            _ => {}
        }
    }
    segments
}
