use cm_capabilities::projection::format_recall_view_at;

use super::fixtures::{browse_fallback_fixture, empty_fixture, search_fixture};

#[test]
fn format_recall_view_drill_down_advisory_fires_on_dominant_kind() {
    // Faceted drill down advisory: the search fixture carries 2/3
    // `decision` rows (66.7%), which clears the 60% dominance
    // threshold, so the trailer must append a `# narrow: cx_recall(...)`
    // line keyed on the dominant kind.
    let (result, request, now) = search_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    let expected = "# narrow: cx_recall(query=\"snippet strategy\", \
                    kinds=[\"decision\"]) - 2 of 3 results are decision";
    assert!(
        rendered.contains(expected),
        "drill-down advisory line missing or malformed:\n{rendered}",
    );
    assert_eq!(
        rendered.matches("# narrow:").count(),
        1,
        "exactly one drill-down advisory expected:\n{rendered}",
    );
}

#[test]
fn format_recall_view_search_fixture_stays_under_2000_bytes() {
    let (result, request, now) = search_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert!(
        rendered.len() <= 2_000,
        "rendered recall search view is {} bytes, exceeds 2000-byte budget:\n{rendered}",
        rendered.len(),
    );
}

#[test]
fn format_recall_view_score_column_omitted_on_non_search_routing() {
    let (result, request, now) = browse_fallback_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    // No normalised score should appear as a leading per row column.
    // The `0.XX` pattern would sit between the short id and the title.
    assert!(
        !rendered.contains("  0."),
        "browse_fallback rendering should not carry a score column:\n{rendered}",
    );
    assert!(
        !rendered.contains("  1.00 "),
        "browse_fallback rendering should not carry a score column:\n{rendered}",
    );
}

#[test]
fn format_recall_view_header_surfaces_per_kind_and_per_tag_histograms() {
    // ALP-1725 acceptance: the recall header must surface both a
    // per kind and a per tag histogram so agents can scan the result
    // set shape without paging through every row.
    let (result, request, now) = search_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert!(
        rendered.contains("\nkinds: decision=2, lesson=1\n"),
        "expected kinds histogram in header:\n{rendered}",
    );
    assert!(
        rendered.contains("\ntags: projection=3, snippet=2, edge-case=1\n"),
        "expected tag histogram in header:\n{rendered}",
    );
}

#[test]
fn format_recall_view_empty_fixture_emits_no_matches_hint() {
    let (result, request, now) = empty_fixture();
    let rendered = format_recall_view_at(&result, &request, now);
    assert!(
        rendered.contains("# no matches"),
        "empty rendering should carry the no-matches hint:\n{rendered}",
    );
    assert!(
        !rendered.contains("cx_get(id="),
        "empty rendering should not emit the cx_get hint:\n{rendered}",
    );
    assert!(rendered.contains("query: "), "\n{rendered}");
    assert!(rendered.contains("routing: search"), "\n{rendered}");
    assert!(rendered.contains("tokens: 0"), "\n{rendered}");
    assert!(rendered.contains("entries:\n  []\n"), "\n{rendered}");
}
