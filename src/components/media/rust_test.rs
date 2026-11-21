use serde_json::json;

use super::*;

fn fixture() -> Value {
    json!({
        "story": {
            "kind": "video", "title": {"ar": "قصتنا", "en": "Our story"},
            "poster": "https://cdn.databayt.org/hogwarts/story.poster.webp",
            "width": 1920, "height": 1080, "duration": 94.5,
            "sources": [
                {"src": "https://cdn.databayt.org/hogwarts/story.av1.mp4", "type": "video/mp4; codecs=\"av01.0.09M.10\""},
                {"src": "https://cdn.databayt.org/hogwarts/story.mp4", "type": "video/mp4; codecs=\"avc1.64002A\""}
            ]
        },
        "add/list-ar": {
            "kind": "image", "width": 2400, "height": 1361,
            "alt": {"ar": "قائمة الطلاب", "en": "The students list"},
            "avif": {"1600": "a1600.avif", "2400": "a2400.avif"},
            "webp": {"2400": "https://cdn.databayt.org/hogwarts/ar/list@2x.webp", "1600": "https://cdn.databayt.org/hogwarts/ar/list.webp"}
        },
        "odd": {"kind": "audio"}
    })
}

#[test]
fn manifest_maps_videos_and_images() {
    let items = map_manifest(&fixture());
    assert_eq!(items.len(), 2);
    let video = items.iter().find(|i| i.id == "story").expect("video");
    assert_eq!(video.kind, "video");
    assert!(
        video.url.ends_with("story.mp4"),
        "h264 preferred: {}",
        video.url
    );
    assert!(video.thumb.ends_with("story.poster.webp"));
    assert_eq!(
        (video.title_ar.as_str(), video.title_en.as_str()),
        ("قصتنا", "Our story")
    );
    assert!((video.duration - 94.5).abs() < f64::EPSILON);
    assert_eq!(video.lang, "");
    let image = items.iter().find(|i| i.id == "add/list-ar").expect("image");
    assert!(
        image.thumb.ends_with("list.webp"),
        "smallest: {}",
        image.thumb
    );
    assert!(
        image.url.ends_with("list@2x.webp"),
        "largest: {}",
        image.url
    );
    assert_eq!(image.title_en, "The students list");
    assert_eq!(image.lang, "ar");
}

#[test]
fn cdn_index_keeps_approved_videos_of_the_brand() {
    let index = json!({"videos": [
        {"id": "H1", "product": "hogwarts", "title": "Tour", "approved": true,
         "urls": {"master": "https://cdn/x/master.mp4", "poster": "https://cdn/x/poster.webp"}},
        {"id": "H2", "product": "hogwarts", "approved": false, "urls": {"master": "https://cdn/y.mp4"}},
        {"id": "M1", "product": "mkan", "approved": true, "urls": {"master": "https://cdn/z.mp4"}}
    ]});
    let items = map_cdn_index(&index, "hogwarts");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, "cdn:H1");
    assert_eq!(items[0].thumb, "https://cdn/x/poster.webp");
    assert!(map_cdn_index(&json!({}), "hogwarts").is_empty());
}

#[test]
fn the_manifest_is_found_beside_but_not_inside_node_modules() {
    let dir = tempfile::tempdir().expect("tmp");
    let repo = dir.path().join("hogwarts");
    let deep = repo.join("src/components/docs");
    std::fs::create_dir_all(&deep).expect("mk");
    std::fs::create_dir_all(repo.join("src/node_modules/x")).expect("mk");
    std::fs::write(repo.join("src/node_modules/x").join(MANIFEST), "{}").expect("w");
    assert_eq!(find_manifest(&repo), None);
    std::fs::write(deep.join(MANIFEST), fixture().to_string()).expect("w");
    assert_eq!(find_manifest(&repo), Some(deep.join(MANIFEST)));
    assert_eq!(load_local(dir.path(), "hogwarts").len(), 2);
    assert!(load_local(dir.path(), "mkan").is_empty());
}

#[test]
fn repo_names_are_validated() {
    for ok in ["hogwarts", "android-app", "a.b_c"] {
        assert!(valid_repo(ok), "{ok}");
    }
    for bad in ["", "../x", "a/b", ".hidden", "a b"] {
        assert!(!valid_repo(bad), "{bad}");
    }
}
