use super::*;
use serde_json::json;

const MARKETS: &str = r#"{"readme":"notes","sd":{"name":{"ar":"السودان","en":"Sudan"},"langs":["ar","en"]},"rw":{"name":{"en":"Rwanda"},"langs":["en"]}}"#;

#[test]
fn markets_map_to_one_item_per_language() {
    let items = markets_items(MARKETS);
    let en: Vec<_> = items.iter().map(|i| i.label_en.as_str()).collect();
    assert_eq!(en, ["Rwanda · English", "Sudan · Arabic", "Sudan · English"]);
    assert_eq!(items[1].label_ar, "السودان · عربي");
    assert_eq!(items[1].set["variant"], "sd-ar");
    assert_eq!(items[1].set["market"], "sd");
    assert_eq!(items[1].set["lang"], "ar");
    // No Arabic name: the English one stands in.
    assert_eq!(items[0].label_ar, "Rwanda · إنجليزي");
    assert!(markets_items("not json").is_empty());
}

#[test]
fn project_ids_are_validated() {
    assert!(valid_project_id("hogwarts-intro-introduction-v001"));
    assert!(valid_project_id("HOG-INTRO-20261120-V001"), "legacy");
    for bad in ["", "ab", "-abc", "../X", "a/b", "A B", "a.b", "Hog-1", "../abc"] {
        assert!(!valid_project_id(bad), "{bad}");
    }
    let home = tempfile::tempdir().unwrap();
    let dir = home.path().join("media/_projects/hog-1");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("project.json"), "{}").unwrap();
    assert!(project_file(home.path(), "hog-1").is_some());
    assert!(project_file(home.path(), "hog-2").is_none());
    assert!(project_file(home.path(), "../hog-1").is_none());
}

#[test]
fn variants_come_from_the_project_and_markets() {
    let project = r#"{"variants":[{"id":"sd-ar","market":"sd","lang":"ar"},{"id":"rw-en"},{"id":"../x"}]}"#;
    let items = variants_items(project, MARKETS);
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].label_ar, "السودان · عربي");
    assert_eq!(items[1].label_en, "Rwanda · English");
    assert_eq!(items[1].set["variant"], "rw-en");
    assert_eq!(items[1].set["market"], "rw");
}

#[test]
fn projects_parse_with_noise_and_label() {
    let out = r#"log line
[{"id":"hog-1","title":"Intro","category":"intro","version":"V001","next":"script","focus":[],"variants":[]},{"id":"bad id"}]"#;
    let items = projects_items(out);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label_en, "intro · V001 — next: script");
    assert_eq!(items[0].label_ar, "intro · V001 — التالي: script");
    assert_eq!(items[0].set["project"], "hog-1");
    assert!(projects_items("oops").is_empty());
}

fn tree() -> Vec<Starter> {
    parse(&[
        json!({"id":"a","label_ar":"أ","label_en":"A","icon":"Film","children":[
            {"id":"a1","label_ar":"1","label_en":"1","set":{"type":"intro","bad key":"x"},
             "source":"markets","then":[{"id":"t","label_ar":"ت","label_en":"T","prompt":"Make {{type}} for {{repo}}"}]},
            {"id":"a2","label_ar":"2","label_en":"2","repos":["mkan"],"prompt":"x"}]}),
        json!({"id":"b","label_ar":"ب","label_en":"B","repos":["hogwarts"],"prompt":"y"}),
        json!({"id":"dead","label_ar":"d","label_en":"d"}),
        json!({"id":"","label_ar":"x","label_en":"x","prompt":"p"}),
        json!({"id":"c","label_ar":"c","label_en":"c","source":"nope","prompt":"p"}),
    ])
}

#[test]
fn starters_parse_nested_with_source_and_then() {
    let t = tree();
    assert_eq!(t.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    let a1 = &t[0].children[0];
    assert_eq!(a1.source, Some(SourceKind::Markets));
    assert_eq!(a1.then[0].prompt.as_deref(), Some("Make {{type}} for {{repo}}"));
    assert_eq!(a1.set.len(), 1, "bad facet keys are dropped");
    assert_eq!(t[0].icon.as_deref(), Some("Film"));
}

#[test]
fn repo_filter_hides_and_prunes() {
    let ids = |v: &[Starter]| v.iter().map(|s| s.id.clone()).collect::<Vec<_>>();
    let hog = filter_repo(tree(), "hogwarts");
    assert_eq!(ids(&hog), ["a", "b"]);
    assert_eq!(ids(&hog[0].children), ["a1"]);
    let mkan = filter_repo(tree(), "mkan");
    assert_eq!(ids(&mkan), ["a"]);
    assert_eq!(ids(&mkan[0].children), ["a1", "a2"]);
    let none = filter_repo(vec![tree().remove(1)], "mkan");
    assert!(none.is_empty());
}

#[test]
fn fills_and_formula_parse() {
    let raw = [
        json!({"id":"f","label_en":"F","fill_en":"Make {{type}}","fill_ar":"  "}),
        json!({"id":"empty","label_en":"E","fill_en":" "}),
    ];
    let t = parse(&raw);
    assert_eq!(t.len(), 1, "a blank fill leads nowhere");
    assert_eq!(t[0].fill_en.as_deref(), Some("Make {{type}}"));
    assert_eq!(t[0].fill_ar, None);
    assert!(t[0].prompt.is_none());
    let f = parse_formula(Some(&json!({"ar":" س ","en":"X {{repo}}"}))).unwrap();
    assert_eq!((f.ar.as_str(), f.en.as_str()), ("س", "X {{repo}}"));
    assert!(parse_formula(Some(&json!({"ar":" ","en":""}))).is_none());
    assert!(parse_formula(Some(&json!("nope"))).is_none());
    assert!(parse_formula(None).is_none());
}
