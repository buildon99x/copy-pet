use super::*;
use crate::clipboard::ClipStore;

fn action(w: &mut Workbench, predicate: impl Fn(Action) -> bool) -> Option<String> {
    w.sel = w
        .actions(Lang::En)
        .iter()
        .position(|(_, a)| predicate(*a))
        .unwrap();
    w.nav(NavKey::Enter, Lang::En, 6)
}
fn request(w: &mut Workbench, text: &str) {
    action(w, |a| matches!(a, Action::Request));
    w.paste(text);
    w.nav(NavKey::Enter, Lang::En, 6);
}
#[test]
fn template_variables_are_literal_and_favorites_keep_the_original_body() {
    let mut w = Workbench::default();
    request(&mut w, "Fix {task}: {task}\nValidate {검증 명령}");
    action(&mut w, |a| matches!(a, Action::SaveFavorite));
    w.paste("Repair");
    w.nav(NavKey::Enter, Lang::En, 6);
    action(&mut w, |a| matches!(a, Action::Variable(0)));
    w.paste("JSON {literal}");
    w.nav(NavKey::Enter, Lang::En, 6);
    action(&mut w, |a| matches!(a, Action::Variable(1)));
    w.paste("cargo test");
    w.nav(NavKey::Enter, Lang::En, 6);
    assert!(w
        .compose(Lang::En)
        .contains("Fix JSON {literal}: JSON {literal}"));
    assert!(w.project().favorites[0].body.contains("{task}"));
    action(&mut w, |a| matches!(a, Action::Review));
    assert!(w.review.is_some());
    w.review = None;
    action(&mut w, |a| matches!(a, Action::Favorite(0)));
    assert!(w.project().values.is_empty());
    assert_eq!(variables(&w.project().request).len(), 2);
}
#[test]
fn multiline_edit_replacement_and_newline_are_bounded() {
    let mut w = Workbench::default();
    request(&mut w, "before");
    action(&mut w, |a| matches!(a, Action::Request));
    w.paste("한글\nnext\tline");
    w.nav(NavKey::PasteText, Lang::En, 6);
    w.input('끝');
    assert_eq!(
        w.project().editor.as_ref().unwrap().text,
        "한글\nnext\tline\n끝"
    );
    w.nav(NavKey::SelectAll, Lang::En, 6);
    w.paste(&"a".repeat(MAX_DRAFT));
    let before = w.project().editor.as_ref().unwrap().text.clone();
    w.input('가');
    assert_eq!(w.project().editor.as_ref().unwrap().text, before);
    w.paste("x");
    assert_eq!(w.project().editor.as_ref().unwrap().text.len(), MAX_DRAFT);
}
#[test]
fn expansion_limit_rejects_repeated_large_value_without_losing_editor() {
    let mut w = Workbench::default();
    request(&mut w, "{x}".repeat(40).as_str());
    action(&mut w, |a| matches!(a, Action::Variable(0)));
    w.paste(&"v".repeat(1024));
    w.nav(NavKey::Enter, Lang::En, 6);
    assert!(w.project().editor.is_some());
    assert!(w.project().values.is_empty());
    assert!(matches!(w.notice, Some(T::Limit)));
}
#[test]
fn section_order_reordering_and_snapshot_survive_history_deletion() {
    let mut store = ClipStore::default();
    store.add_copy("first".into(), None);
    let id = store.visible_filtered("", None)[0].id;
    let mut w = Workbench::default();
    w.add_clip(store.get(id).unwrap());
    store.add_copy("second".into(), None);
    w.add_clip(store.visible_filtered("", None)[0]);
    store.delete(id);
    w.sel = w
        .actions(Lang::En)
        .iter()
        .position(|(_, a)| matches!(a, Action::Item(1)))
        .unwrap();
    w.nav(NavKey::Pin, Lang::En, 6);
    assert_eq!(w.project().items[0].text, "second");
    w.nav(NavKey::Right, Lang::En, 6);
    let text = w.compose(Lang::En);
    assert!(text.find("first").unwrap() < text.find("second").unwrap());
    assert!(text.contains("## Errors"));
    w.nav(NavKey::Delete, Lang::En, 6);
    assert_eq!(w.project().items.len(), 1);
}
#[test]
fn project_switch_retains_an_unfinished_editor_and_restart_state() {
    let mut w = Workbench::default();
    action(&mut w, |a| matches!(a, Action::Request));
    w.paste("unfinished\n한글");
    w.choose_projects();
    action(&mut w, |a| matches!(a, Action::NewProject));
    request(&mut w, "other project");
    w.choose_projects();
    action(&mut w, |a| matches!(a, Action::Project(0)));
    assert!(w.editing());
    assert_eq!(
        w.project().editor.as_ref().unwrap().text,
        "unfinished\n한글"
    );
    let saved = serde_json::to_string(&w).unwrap();
    let restored: Workbench = serde_json::from_str(&saved).unwrap();
    assert!(restored.valid());
    assert_eq!(restored.active, 0);
    assert_eq!(restored.projects[1].request, "other project");
    assert_eq!(
        restored.project().editor.as_ref().unwrap().text,
        "unfinished\n한글"
    );
    assert!(!restored.open);
    assert!(restored.review.is_none());
}
#[test]
fn tray_default_tags_merge_case_insensitively_with_original_priority() {
    let mut store = ClipStore::default();
    store.add_copy("clip".into(), None);
    let id = store.visible_filtered("", None)[0].id;
    store.set_tags(id, "A b c d e f g h");
    let mut w = Workbench::default();
    w.project_mut().tags = vec!["a".into(), "extra".into()];
    w.add_clip(store.get(id).unwrap());
    assert_eq!(w.project().items[0].tags.len(), 8);
    assert!(w.valid());
    assert_eq!(store.get(id).unwrap().tags.len(), 8);
}
#[test]
fn image_snapshots_only_emit_attachment_guidance() {
    let mut w = Workbench::default();
    w.project_mut().items.push(Item {
        text: String::new(),
        image: true,
        section: 1,
        tags: vec![],
    });
    let text = w.compose(Lang::En);
    assert!(text.contains("attach the original separately"));
    assert!(!text.contains("base64"));
}
#[test]
fn secret_review_redacts_prefixes_bearer_assignments_and_pem_with_unicode() {
    let token = format!("sk-{}", "x".repeat(24));
    let input=format!("한글\nOPENAI_API_KEY=\"{token}\"\nAuthorization: Bearer abcdefgh12345678\npassword = 'sample-pass'\n-----BEGIN PRIVATE KEY-----\nSYNTHETIC\n-----END PRIVATE KEY-----\n");
    let review = redact(&input, Lang::En);
    assert_eq!(review.findings.len(), 4);
    assert!(!review.text.contains(&token));
    assert!(!review.text.contains("SYNTHETIC"));
    assert!(!review.findings.join(" ").contains("sample-pass"));
    assert!(review.text.starts_with("한글\n"));
    assert!(input.contains("SYNTHETIC"));
}
#[test]
fn ordinary_prose_and_code_are_not_reported_as_secrets_or_variables() {
    let text = "fn main() { println!(\"hello\"); }\nlet token_count = 1234;\nAPI_KEY=\n";
    assert!(redact(text, Lang::En).findings.is_empty());
    assert!(variables(text).is_empty());
    assert_eq!(
        variables("{task} {task} {작업 내용}"),
        vec!["task", "작업 내용"]
    );
}
#[test]
fn review_needs_explicit_confirmation_and_cancellation_keeps_originals() {
    let mut w = Workbench::default();
    request(&mut w, "API_KEY=synthetic-only-value");
    assert!(action(&mut w, |a| matches!(a, Action::Review)).is_none());
    let original = w.project().request.clone();
    assert!(w.review.is_some());
    w.nav(NavKey::Esc, Lang::En, 6);
    assert!(w.review.is_none());
    assert_eq!(w.project().request, original);
    action(&mut w, |a| matches!(a, Action::Review));
    let copied = action(&mut w, |a| matches!(a, Action::Confirm)).unwrap();
    assert!(copied.contains("[REDACTED_1]"));
    assert_eq!(w.project().request, original);
}
#[test]
fn validation_rejects_malformed_loaded_fields_and_payload_budget() {
    let mut w = Workbench::default();
    assert!(w.valid());
    w.project_mut().items.push(Item {
        text: "ok".into(),
        image: false,
        section: 99,
        tags: vec![],
    });
    assert!(!w.valid());
    w.project_mut().items[0].section = 0;
    w.project_mut().items[0].tags = vec!["one two".into()];
    assert!(!w.valid());
    w.project_mut().items.clear();
    w.project_mut().request = "x".repeat(MAX_DRAFT + 1);
    assert!(!w.valid());
    w.project_mut().request.clear();
    w.project_mut().items.push(Item {
        text: "x".repeat(MAX_TEXT),
        image: false,
        section: 0,
        tags: vec![],
    });
    assert!(!w.valid());
}
#[test]
fn maximum_project_payloads_fit_the_file_budget_even_with_json_escapes() {
    let projects = (0..MAX_PROJECTS)
        .map(|i| Project {
            name: format!("P{i}"),
            items: vec![Item {
                text: "\0".repeat(MAX_TEXT - 64),
                image: false,
                section: 0,
                tags: vec![],
            }],
            ..Project::default()
        })
        .collect();
    let w = Workbench {
        projects,
        ..Workbench::default()
    };
    assert!(w.valid());
    assert!(serde_json::to_string(&w).unwrap().len() < MAX_FILE);
}

#[test]
fn template_reuse_checks_the_final_project_budget_and_preserves_existing_content() {
    let mut w = Workbench::default();
    w.project_mut().favorites.push(Favorite {
        name: "large".into(),
        body: "f".repeat(MAX_DRAFT),
    });
    let used = project_bytes(w.project());
    w.project_mut().items.push(Item {
        text: "c".repeat(MAX_TEXT - used - 8),
        image: false,
        section: 1,
        tags: vec![],
    });
    assert!(w.valid());
    action(&mut w, |a| matches!(a, Action::Favorite(0)));
    assert!(w.project().request.is_empty());
    assert!(matches!(w.notice, Some(T::Limit)));
    assert!(w.valid());
}

#[test]
fn measured_review_wrapping_preserves_all_characters_at_minimum_panel_width() {
    let text = "한글 검토 문구입니다. ".repeat(15) + "\n    indented  code\twith tabs";
    let w = Workbench {
        text_width: crate::panel::MIN_W - 40.0,
        review: Some(redact(&text, Lang::Ko)),
        ..Workbench::default()
    };
    let actions = w.actions(Lang::Ko);
    let first = actions
        .iter()
        .position(|(_, a)| matches!(a, Action::Back))
        .unwrap()
        + 1;
    let parts: Vec<_> = actions[first..].iter().map(|(s, _)| s.as_str()).collect();
    assert_eq!(parts.concat(), text.replace('\n', ""));
    assert!(parts
        .iter()
        .all(|s| crate::sysfont::measure(s, BODY_FONT) <= w.text_width));
    let view = w.view(Lang::Ko);
    assert_eq!(view.rows[first..].concat(), parts.concat());
}

#[test]
fn saved_file_reopens_projects_and_unfinished_multiline_draft() {
    let path =
        std::env::temp_dir().join(format!("clipcat-ai-roundtrip-{}.json", std::process::id()));
    let mut w = Workbench::default();
    w.projects.push(Project {
        name: "Second".into(),
        ..Project::default()
    });
    w.active = 1;
    action(&mut w, |a| matches!(a, Action::Request));
    w.paste("not yet saved by Enter\n미완성");
    assert!(crate::state::write_atomic(
        &path,
        &serde_json::to_string(&w).unwrap()
    ));
    let reopened = Workbench::load_from(&path);
    assert_eq!(reopened.active, 1);
    assert_eq!(
        reopened.project().editor.as_ref().unwrap().text,
        "not yet saved by Enter\n미완성"
    );
    let broken = "{\"projects\":[]}";
    std::fs::write(&path, broken).unwrap();
    let recovered = Workbench::load_from(&path);
    assert!(recovered.valid());
    assert_eq!(std::fs::read_to_string(&path).unwrap(), broken);
    std::fs::remove_file(path).unwrap();
}

#[test]
fn long_editor_scrolls_without_changing_draft_and_new_input_follows_the_end() {
    let mut w = Workbench::default();
    action(&mut w, |a| matches!(a, Action::Request));
    let text = (1..=60)
        .map(|i| format!("line {i:02}"))
        .collect::<Vec<_>>()
        .join("\n");
    w.paste(&text);
    let visible = |w: &Workbench| {
        let view = w.view(Lang::En);
        let start = view.visible_start(8);
        view.rows[start..]
            .iter()
            .take(8)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(visible(&w).last().unwrap(), "line 60");
    w.wheel(-8, 8, Lang::En);
    assert_eq!(visible(&w).last().unwrap(), "line 52");
    w.nav(NavKey::Home, Lang::En, 8);
    assert_eq!(visible(&w)[0], "line 01");
    w.nav(NavKey::PageDown, Lang::En, 8);
    assert_eq!(visible(&w)[0], "line 09");
    w.nav(NavKey::PageUp, Lang::En, 8);
    assert_eq!(visible(&w)[0], "line 01");
    w.nav(NavKey::End, Lang::En, 8);
    assert_eq!(visible(&w).last().unwrap(), "line 60");
    w.nav(NavKey::Home, Lang::En, 8);
    w.input('!');
    assert_eq!(visible(&w).last().unwrap(), "line 60!");
    w.nav(NavKey::Home, Lang::En, 8);
    w.paste("\nlast");
    assert_eq!(visible(&w).last().unwrap(), "last");
    w.nav(NavKey::Home, Lang::En, 8);
    w.nav(NavKey::Backspace, Lang::En, 8);
    assert_eq!(visible(&w).last().unwrap(), "las");
    w.nav(NavKey::Enter, Lang::En, 8);
    assert_eq!(w.project().request, text + "!\nlas");
}
