mod common;

use cm_capabilities::recall::{RecallRequest, RecallRouting, recall};
use cm_capabilities::scope::ScopeSelector;
use cm_core::{Confidence, EntryKind, EntryMeta, ScopePath};

use common::{CANONICAL_CONTEXT_REPO_SCOPE, seed_entry_with_meta, test_store};

struct EnvGuard {
    cwd: std::path::PathBuf,
    ranking: Option<String>,
}

impl EnvGuard {
    fn enter(cwd: &std::path::Path) -> Self {
        let guard = Self {
            cwd: std::env::current_dir().unwrap(),
            ranking: std::env::var("CM_RECALL_RANKING").ok(),
        };
        std::env::set_current_dir(cwd).unwrap();
        unsafe {
            std::env::remove_var("CM_RECALL_RANKING");
        }
        guard
    }

    fn set_ranking_mode(mode: &str) {
        unsafe {
            std::env::set_var("CM_RECALL_RANKING", mode);
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        std::env::set_current_dir(&self.cwd).unwrap();
        unsafe {
            match &self.ranking {
                Some(value) => std::env::set_var("CM_RECALL_RANKING", value),
                None => std::env::remove_var("CM_RECALL_RANKING"),
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn recall_ranking_mode_resolves_default_config_and_env_precedence() {
    let clean_cwd = tempfile::tempdir().unwrap();
    let _env = EnvGuard::enter(clean_cwd.path());
    let (store, _db_dir) = test_store().await;

    seed_entry_with_meta(
        &store,
        "Global high priority",
        "rankneedle",
        EntryKind::Fact,
        "global",
        EntryMeta {
            confidence: Some(Confidence::Medium),
            priority: Some(10),
            ..Default::default()
        },
    )
    .await;
    seed_entry_with_meta(
        &store,
        "Repo default priority",
        "rankneedle",
        EntryKind::Fact,
        CANONICAL_CONTEXT_REPO_SCOPE,
        EntryMeta {
            confidence: Some(Confidence::Medium),
            priority: Some(0),
            ..Default::default()
        },
    )
    .await;

    let request = RecallRequest {
        query: Some("rankneedle".to_owned()),
        scope: Some(ScopeSelector::Path(
            ScopePath::parse(CANONICAL_CONTEXT_REPO_SCOPE).unwrap(),
        )),
        limit: 10,
        ..Default::default()
    };

    let legacy = recall(&store, request.clone()).await.unwrap();
    assert_eq!(legacy.routing, RecallRouting::Search);
    assert_eq!(legacy.entries[0].entry.title, "Repo default priority");
    assert_eq!(legacy.entries[1].entry.title, "Global high priority");

    std::fs::write(
        clean_cwd.path().join(cm_core::CM_CONFIG_FILENAME),
        "[recall]\nranking_mode = \"live\"\n",
    )
    .unwrap();
    let config_live = recall(&store, request.clone()).await.unwrap();
    assert_eq!(config_live.routing, RecallRouting::Search);
    assert_eq!(config_live.entries[0].entry.title, "Global high priority");
    assert_eq!(config_live.entries[1].entry.title, "Repo default priority");

    EnvGuard::set_ranking_mode("legacy");
    let env_legacy = recall(&store, request.clone()).await.unwrap();
    assert_eq!(env_legacy.routing, RecallRouting::Search);
    assert_eq!(env_legacy.entries[0].entry.title, "Repo default priority");
    assert_eq!(env_legacy.entries[1].entry.title, "Global high priority");

    EnvGuard::set_ranking_mode("live");
    let live = recall(&store, request).await.unwrap();
    assert_eq!(live.routing, RecallRouting::Search);
    assert_eq!(live.entries[0].entry.title, "Global high priority");
    assert_eq!(live.entries[1].entry.title, "Repo default priority");
}
