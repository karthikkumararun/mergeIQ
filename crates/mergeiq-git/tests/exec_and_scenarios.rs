mod support;
use support::Scenario;

#[test]
fn scenario_builder_produces_merge_conflict() {
    let s = Scenario::new();
    s.merge_conflict();
    assert!(!s.git(&["ls-files", "-u"]).is_empty());
}
