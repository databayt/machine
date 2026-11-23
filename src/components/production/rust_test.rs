use super::*;

#[test]
fn ids_and_args_are_validated() {
    assert!(valid_film_id("hogwarts-intro-introduction-v001"));
    assert!(!valid_film_id("../x") && !valid_film_id("-rf") && !valid_film_id("a b") && !valid_film_id(""));
    assert_eq!(status_args("a-b").unwrap(), ["status", "a-b", "--json"]);
    assert!(status_args("a b").is_err());
    assert_eq!(list_args("hogwarts").unwrap(), ["list", "--json", "--repo", "hogwarts"]);
    assert_eq!(list_args("").unwrap(), ["list", "--json"]);
    assert!(list_args("a b").is_err());
}

#[test]
fn json_is_found_after_noise() {
    assert_eq!(json_of("warning\n{\"a\":1}").unwrap()["a"], 1);
    assert!(json_of("[ {\"id\":\"x\"} ]").unwrap().is_array());
    assert!(json_of("not json").is_none());
}

#[test]
fn base64_matches_the_standard() {
    assert_eq!(base64(b""), "");
    assert_eq!(base64(b"f"), "Zg==");
    assert_eq!(base64(b"fo"), "Zm8=");
    assert_eq!(base64(b"foo"), "Zm9v");
    assert_eq!(base64(b"foobar"), "Zm9vYmFy");
}

#[test]
fn status_reads_the_two_files_under_home() {
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("media/_projects");
    let film = root.join("f-1");
    fs::create_dir_all(film.join("04-storyboard")).unwrap();
    fs::create_dir_all(film.join("05-assets")).unwrap();
    fs::write(film.join("04-storyboard/shot-list.json"), r#"{"scenes":[{"id":"s1"}]}"#).unwrap();
    // The asset manifest is absent: it reads as null, not as an error.
    let v = status_in(r#"{"id":"f-1","stages":{"request":true}}"#, "f-1", home.path(), &root);
    assert_eq!(v["status"]["id"], "f-1");
    assert_eq!(v["shotList"]["scenes"][0]["id"], "s1");
    assert!(v["assetManifest"].is_null());
    // A status that is not JSON is null.
    assert!(status_in("nope", "f-1", home.path(), &root)["status"].is_null());
}

#[test]
fn a_named_path_outside_home_is_never_read() {
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("shot.json"), "{\"scenes\":[]}").unwrap();
    let stdout = format!(r#"{{"id":"f","shotList":{:?}}}"#, outside.path().join("shot.json").display().to_string());
    let v = status_in(&stdout, "f", home.path(), &home.path().join("p"));
    assert!(v["shotList"].is_null());
}

fn kit_fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let kun = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    fs::create_dir_all(kun.path().join("content/media")).unwrap();
    fs::create_dir_all(kun.path().join("public/brands")).unwrap();
    fs::write(kun.path().join("public/brands/b.png"), b"\x89PNG").unwrap();
    fs::write(
        kun.path().join("content/media/brand-kit.json"),
        r##"{"production":{"balqalam":{"status":"ready","repo":"hogwarts","logo":"public/brands/b.png",
            "fonts":{"latin":"Geist"},"voice":"house-ar","video":{"canvas":"#faf9f5","numerals":"arab","ink":"#141413"}}}}"##,
    )
    .unwrap();
    (kun, home)
}

#[test]
fn brand_comes_from_the_kit() {
    let (kun, home) = kit_fixture();
    // The data-URL check needs the logo under home; here it sits under kun, so no thumbnail, but the path resolves.
    let v = brand_in(kun.path(), home.path(), "balqalam").unwrap();
    assert_eq!(v["voice"], "house-ar");
    assert_eq!(v["fonts"]["latin"], "Geist");
    assert_eq!(v["palette"]["canvas"], "#faf9f5");
    assert!(v["palette"].get("numerals").is_none(), "only colours are swatches");
    assert!(v["logo"].as_str().unwrap().ends_with("public/brands/b.png"));
    assert!(brand_in(kun.path(), home.path(), "nope").is_err());
    assert!(brand_in(kun.path(), home.path(), "../x").is_err());
}

#[test]
fn thumbs_are_images_under_home_within_the_cap() {
    let home = tempfile::tempdir().unwrap();
    let png = home.path().join("a.png");
    fs::write(&png, b"foo").unwrap();
    assert_eq!(thumb_of(&png, home.path()).unwrap(), "data:image/png;base64,Zm9v");
    let txt = home.path().join("a.txt");
    fs::write(&txt, b"foo").unwrap();
    assert!(thumb_of(&txt, home.path()).is_none());
    let other = tempfile::tempdir().unwrap();
    let outside = other.path().join("a.png");
    fs::write(&outside, b"foo").unwrap();
    assert!(thumb_of(&outside, home.path()).is_none());
    let big = home.path().join("big.png");
    fs::write(&big, vec![0u8; usize::try_from(MAX_THUMB_BYTES).unwrap() + 1]).unwrap();
    assert!(thumb_of(&big, home.path()).is_none());
}

#[test]
fn exist_marks_files_under_home_only() {
    let home = tempfile::tempdir().unwrap();
    let there = home.path().join("a.png");
    fs::write(&there, b"x").unwrap();
    let got = exist_in(&[there.display().to_string(), home.path().join("nope.png").display().to_string(), "/etc/hosts".to_owned()], home.path());
    assert_eq!(got, [true, false, false]);
}

fn sets(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
    pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
}

#[test]
fn lock_args_build_both_forms() {
    let none = BTreeMap::new();
    assert_eq!(
        lock_args("f-1", None, true, &none, None, "abdout").unwrap(),
        ["lock", "f-1", "--all", "--by", "abdout", "--no-sync", "--json"]
    );
    assert_eq!(
        lock_args("f-1", Some("visual"), false, &sets(&[("look", "mixed"), ("threed", "blender 5")]), None, "abdout").unwrap(),
        ["lock", "f-1", "visual", "--set", "look=mixed", "--set", "threed=blender 5", "--by", "abdout", "--no-sync", "--json"]
    );
    let f = Path::new("/tmp/s.json");
    assert_eq!(
        lock_args("f-1", Some("edit"), false, &none, Some(f), "abdout").unwrap(),
        ["lock", "f-1", "edit", "--spec", "/tmp/s.json", "--by", "abdout", "--no-sync", "--json"]
    );
}

#[test]
fn lock_args_refuse_bad_input() {
    let none = BTreeMap::new();
    assert!(lock_args("f-1", Some("edit"), true, &none, None, "abdout").is_err());
    assert!(lock_args("f-1", None, false, &none, None, "abdout").is_err());
    assert!(lock_args("f-1", Some("editing"), false, &none, None, "abdout").is_err());
    assert!(lock_args("f-1", Some("edit"), false, &sets(&[("Look", "x")]), None, "abdout").is_err());
    assert!(lock_args("f-1", Some("edit"), false, &sets(&[("look", "a\nb")]), None, "abdout").is_err());
    assert!(lock_args("f-1", Some("edit"), false, &sets(&[("look", "")]), None, "abdout").is_err());
    assert!(lock_args("f-1", Some("edit"), false, &sets(&[("look", &"x".repeat(301))]), None, "abdout").is_err());
    assert!(lock_args("f-1", None, true, &none, Some(Path::new("/tmp/s.json")), "abdout").is_err());
    assert!(lock_args("../x", None, true, &none, None, "abdout").is_err());
    assert!(lock_args("f-1", None, true, &none, None, "Bad Login").is_err());
    assert!(lock_args("f-1", None, true, &none, None, "").is_err());
}

#[test]
fn spec_and_plan_text_are_checked() {
    assert!(check_spec(r#"{"stage":"visual","decisions":{"look":"mixed"}}"#, "visual").is_ok());
    assert!(check_spec(r#"{"stage":"audio"}"#, "visual").is_err());
    assert!(check_spec(r#"{"decisions":{}}"#, "visual").is_err());
    assert!(check_spec("[1]", "visual").is_err());
    assert!(check_spec("not json", "visual").is_err());
    let big = format!(r#"{{"stage":"visual","notes":"{}"}}"#, "x".repeat(65 * 1024));
    assert!(check_spec(&big, "visual").is_err());

    assert!(check_plan(r#"{"picks":{"kind":"intro"}}"#).is_ok());
    assert!(check_plan("[]").is_err());
    assert!(check_plan("nope").is_err());
    let big = format!(r#"{{"notes":"{}"}}"#, "x".repeat(257 * 1024));
    assert!(check_plan(&big).is_err());
}

#[test]
fn saved_paths_sit_under_the_given_folder() {
    let dir = Path::new("/home/x/.kun/machine/video/plans");
    let p = plan_path(dir, 1_700_000_000_123);
    assert!(p.starts_with(dir));
    assert_eq!(p.file_name().unwrap(), "1700000000123.json");
    let s = spec_path(Path::new("/d"), "f-1", "visual", 5);
    assert_eq!(s, Path::new("/d/f-1-visual-5.json"));
}

#[test]
fn the_kun_checkout_is_tried_first_for_lock() {
    let home = tempfile::tempdir().unwrap();
    let kun = tempfile::tempdir().unwrap();
    for root in [home.path(), kun.path()] {
        let f = root.join(".claude/skills").join(SCRIPT);
        fs::create_dir_all(f.parent().unwrap()).unwrap();
        fs::write(f, "").unwrap();
    }
    assert!(script_path_kun_first(home.path(), kun.path()).unwrap().starts_with(kun.path()));
    assert!(script_path(home.path(), kun.path()).unwrap().starts_with(home.path()));
}
