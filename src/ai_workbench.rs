//! Local, explicit AI request preparation. Projects own bounded text snapshots;
//! no provider client, global-input observer, or network endpoint is involved.
use crate::clipboard::{parse_tags, Clip, MAX_TEXT};
use crate::i18n::{ai, AiText as T, Lang};
use crate::panel::NavKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_ITEMS: usize = 40;
const MAX_PROJECTS: usize = 24;
const MAX_FAVORITES: usize = 24;
const MAX_DRAFT: usize = 32 * 1024;
const MAX_FILE: usize = 64 * 1024 * 1024;

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Project {
    pub name: String,
    pub tags: Vec<String>,
    pub request: String,
    pub values: BTreeMap<String, String>,
    pub items: Vec<Item>,
    pub favorites: Vec<Favorite>,
    pub editor: Option<Editor>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Item {
    pub text: String,
    pub image: bool,
    pub section: usize,
    pub tags: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Favorite {
    pub name: String,
    pub body: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Editor {
    pub kind: EditKind,
    pub text: String,
}
#[derive(Clone, Serialize, Deserialize)]
pub enum EditKind {
    Project,
    Tags,
    Request,
    FavoriteName,
    Variable(String),
}

#[derive(Serialize, Deserialize)]
#[serde(default)]
pub struct Workbench {
    pub projects: Vec<Project>,
    pub active: usize,
    #[serde(skip)]
    pub dirty: bool,
    #[serde(skip)]
    pub open: bool,
    #[serde(skip)]
    pub adding: bool,
    #[serde(skip)]
    pub sel: usize,
    #[serde(skip)]
    pub scroll: usize,
    #[serde(skip)]
    pub text_width: f32,
    #[serde(skip)]
    select_all: bool,
    #[serde(skip)]
    choosing_project: bool,
    #[serde(skip)]
    pub review: Option<Review>,
    #[serde(skip)]
    pub notice: Option<T>,
}
impl Default for Workbench {
    fn default() -> Self {
        Self {
            projects: vec![Project {
                name: "Default".into(),
                ..Project::default()
            }],
            active: 0,
            dirty: false,
            open: false,
            adding: false,
            sel: 0,
            scroll: 0,
            text_width: 232.0,
            select_all: false,
            choosing_project: false,
            review: None,
            notice: None,
        }
    }
}
#[derive(Clone)]
pub struct Review {
    pub text: String,
    pub findings: Vec<String>,
}
#[derive(Clone)]
pub struct View {
    pub editing: bool,
    pub title: String,
    pub input: String,
    pub hint: String,
    pub rows: Vec<String>,
    pub selected: usize,
    pub scroll: usize,
}
#[derive(Clone, Copy)]
enum Action {
    Projects,
    NewProject,
    Rename,
    Project(usize),
    Tags,
    Request,
    Add,
    SaveFavorite,
    Favorite(usize),
    Variable(usize),
    Item(usize),
    Review,
    Confirm,
    Back,
    Clear,
    None,
}

impl Workbench {
    pub fn load() -> Self {
        let Some(path) = crate::state::config_dir().map(|p| p.join("requests.json")) else {
            return Self::default();
        };
        Self::load_from(&path)
    }
    fn load_from(path: &std::path::Path) -> Self {
        // Refuse huge or invalid files, retaining them on disk for recovery.
        let mut value = std::fs::metadata(path)
            .ok()
            .filter(|m| m.len() <= MAX_FILE as u64)
            .and_then(|_| std::fs::read_to_string(path).ok())
            .and_then(|s| serde_json::from_str::<Self>(&s).ok())
            .unwrap_or_default();
        if value.projects.is_empty() || value.projects.len() > MAX_PROJECTS {
            return Self::default();
        }
        value.active = value.active.min(value.projects.len() - 1);
        if !value.valid() {
            return Self::default();
        }
        value
    }
    fn valid(&self) -> bool {
        !self.projects.is_empty()
            && self.projects.len() <= MAX_PROJECTS
            && self.active < self.projects.len()
            && self.projects.iter().all(|p| {
                !p.name.trim().is_empty()
                    && p.name.chars().count() <= 60
                    && !p.name.chars().any(char::is_control)
                    && project_bytes(p) <= MAX_TEXT
                    && p.request.len() <= MAX_DRAFT
                    && p.items.len() <= MAX_ITEMS
                    && p.items.iter().map(|i| i.text.len()).sum::<usize>() <= MAX_TEXT
                    && p.items
                        .iter()
                        .all(|i| i.section < 4 && valid_tags(&i.tags) && i.text.len() <= MAX_TEXT)
                    && valid_tags(&p.tags)
                    && p.favorites.len() <= MAX_FAVORITES
                    && p.favorites.iter().all(|f| {
                        !f.name.trim().is_empty()
                            && f.name.chars().count() <= 60
                            && !f.name.chars().any(char::is_control)
                            && f.body.len() <= MAX_DRAFT
                    })
                    && p.values.values().map(String::len).sum::<usize>() <= MAX_DRAFT
                    && p.values
                        .iter()
                        .all(|(k, v)| variables(&p.request).contains(k) && v.len() <= MAX_DRAFT)
                    && expanded(&p.request, &p.values).is_some()
                    && p.editor.as_ref().is_none_or(|e| {
                        e.text.len() <= MAX_DRAFT
                            && match &e.kind {
                                EditKind::Variable(key) => variables(&p.request).contains(key),
                                _ => true,
                            }
                    })
            })
    }
    pub fn save(&mut self) {
        if !self.dirty {
            return;
        }
        if let (Some(path), Ok(json)) = (
            crate::state::config_dir().map(|p| p.join("requests.json")),
            serde_json::to_string(self),
        ) {
            if self.valid() && json.len() <= MAX_FILE {
                if crate::state::write_atomic(&path, &json) {
                    self.dirty = false;
                } else {
                    self.notice = Some(T::SaveFailed);
                }
            } else {
                self.notice = Some(T::Limit);
            }
        } else {
            self.notice = Some(T::SaveFailed);
        }
    }
    fn room(&self, extra: usize) -> bool {
        project_bytes(self.project()).saturating_add(extra) <= MAX_TEXT
    }
    pub fn editing(&self) -> bool {
        self.project().editor.is_some() && !self.choosing_project && self.review.is_none()
    }
    pub fn choose_projects(&mut self) {
        self.choosing_project = true;
        self.review = None;
        self.sel = 0;
        self.scroll = 0;
    }
    pub fn paste(&mut self, text: &str) {
        if !self.room(text.len().saturating_sub(if self.select_all {
            self.project().editor.as_ref().map_or(0, |e| e.text.len())
        } else {
            0
        })) {
            self.notice = Some(T::Limit);
            return;
        }
        let selected = std::mem::take(&mut self.select_all);
        if let Some(e) = &mut self.project_mut().editor {
            let base = if selected { 0 } else { e.text.len() };
            let clean: String = text
                .chars()
                .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
                .collect();
            if base + clean.len() > MAX_DRAFT {
                self.notice = Some(T::Limit);
                return;
            }
            if selected {
                e.text.clear();
            }
            e.text.push_str(&clean);
            self.dirty = true;
        }
    }
    pub fn project(&self) -> &Project {
        &self.projects[self.active]
    }
    fn project_mut(&mut self) -> &mut Project {
        &mut self.projects[self.active]
    }
    pub fn toggle(&mut self) {
        self.open = !self.open;
        self.adding = false;
        self.review = None;
        self.sel = 0;
        self.scroll = 0;
    }
    pub fn add_clip(&mut self, clip: &Clip) {
        if !self.room(clip.text.len() + clip.tags.iter().map(String::len).sum::<usize>() + 1024) {
            self.notice = Some(T::Limit);
            return;
        }
        let p = self.project_mut();
        if p.items.len() >= MAX_ITEMS
            || p.items.iter().map(|i| i.text.len()).sum::<usize>() + clip.text.len() > MAX_TEXT
        {
            self.notice = Some(T::Limit);
            return;
        }
        let mut tags = parse_tags(&clip.tags.join(", ")).unwrap_or_default();
        for tag in &p.tags {
            let candidate = format!("{},{}", tags.join(","), tag);
            if let Some(normalized) = parse_tags(&candidate) {
                tags = normalized;
            }
        }
        p.items.push(Item {
            text: clip.text.clone(),
            image: clip.image.is_some(),
            section: 1,
            tags,
        });
        self.dirty = true;
        self.notice = Some(T::Added);
    }
    pub fn compose(&self, lang: Lang) -> String {
        let p = self.project();
        let mut out = String::new();
        for section in 0..4 {
            let mut body = Vec::new();
            if section == 0 && !p.request.trim().is_empty() {
                body.push(expanded(&p.request, &p.values).unwrap_or_default());
            }
            for item in p.items.iter().filter(|i| i.section == section) {
                if item.image {
                    body.push(ai(lang, T::Image).into());
                } else {
                    body.push(item.text.clone());
                }
            }
            if !body.is_empty() {
                out.push_str("## ");
                out.push_str(ai(lang, section_label(section)));
                out.push('\n');
                out.push_str(&body.join("\n\n"));
                out.push_str("\n\n");
            }
        }
        out.trim_end().into()
    }
    fn begin(&mut self, kind: EditKind, text: String) {
        if !self.room(text.len() + 256) {
            self.notice = Some(T::Limit);
            return;
        }
        self.project_mut().editor = Some(Editor { kind, text });
        self.select_all = true;
        self.dirty = true;
        self.review = None;
    }
    pub fn input(&mut self, c: char) {
        if c.is_control() || !self.editing() {
            return;
        }
        if !self.room(c.len_utf8().saturating_sub(if self.select_all {
            self.project().editor.as_ref().map_or(0, |e| e.text.len())
        } else {
            0
        })) {
            self.notice = Some(T::Limit);
            return;
        }
        let selected = std::mem::take(&mut self.select_all);
        if let Some(e) = &mut self.project_mut().editor {
            if selected {
                e.text.clear();
            }
            if e.text.len() + c.len_utf8() <= MAX_DRAFT {
                e.text.push(c);
                self.dirty = true;
            }
        }
    }
    fn finish(&mut self) {
        let Some(e) = self.project().editor.clone() else {
            return;
        };
        match &e.kind {
            EditKind::Project => {
                let name = e.text.trim();
                if name.is_empty()
                    || name.chars().count() > 60
                    || name.chars().any(char::is_control)
                {
                    self.notice = Some(T::Invalid);
                    return;
                }
                self.project_mut().name = name.into();
                self.project_mut().editor = None;
            }
            EditKind::Tags => {
                let Some(tags) = parse_tags(&e.text) else {
                    self.notice = Some(T::Invalid);
                    return;
                };
                self.project_mut().tags = tags;
                self.project_mut().editor = None;
            }
            EditKind::Request => {
                self.project_mut().request = e.text;
                self.project_mut().values.clear();
                self.project_mut().editor = None;
            }
            EditKind::FavoriteName => {
                if e.text.trim().is_empty()
                    || e.text.chars().count() > 60
                    || e.text.chars().any(char::is_control)
                    || self.project().request.trim().is_empty()
                    || self.project().favorites.len() >= MAX_FAVORITES
                {
                    self.notice = Some(T::Invalid);
                    return;
                }
                if !self.room(self.project().request.len() + 64) {
                    self.notice = Some(T::Limit);
                    return;
                }
                let body = self.project().request.clone();
                self.project_mut().favorites.push(Favorite {
                    name: e.text.trim().into(),
                    body,
                });
                self.project_mut().editor = None;
            }
            EditKind::Variable(key) => {
                // Values are literal: braces entered in a value are never evaluated.
                let mut values = self.project().values.clone();
                values.insert(key.clone(), e.text);
                if values.values().map(String::len).sum::<usize>() > MAX_DRAFT
                    || expanded(&self.project().request, &values).is_none()
                {
                    self.notice = Some(T::Limit);
                    return;
                }
                self.project_mut().values = values;
                self.project_mut().editor = None;
            }
        }
        self.dirty = true;
        self.select_all = false;
        self.notice = None;
    }
    fn actions(&self, lang: Lang) -> Vec<(String, Action)> {
        if let Some(review) = &self.review {
            let mut rows: Vec<_> = wrap(ai(lang, T::Warning), self.text_width)
                .into_iter()
                .map(|s| (s, Action::None))
                .collect();
            rows.push((
                format!("{} ({})", ai(lang, T::Findings), review.findings.len()),
                Action::None,
            ));
            rows.extend(
                review
                    .findings
                    .iter()
                    .flat_map(|s| wrap(s, self.text_width))
                    .map(|s| (s, Action::None)),
            );
            rows.push((ai(lang, T::Confirm).into(), Action::Confirm));
            rows.push((ai(lang, T::Back).into(), Action::Back));
            rows.extend(
                review
                    .text
                    .lines()
                    .flat_map(|line| wrap(line, self.text_width))
                    .map(|s| (s, Action::None)),
            );
            return rows;
        }
        if self.choosing_project {
            let mut rows: Vec<_> = self
                .projects
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    (
                        format!("{} {}", if i == self.active { "✓" } else { " " }, p.name),
                        Action::Project(i),
                    )
                })
                .collect();
            rows.push((ai(lang, T::NewProject).into(), Action::NewProject));
            rows.push((ai(lang, T::Back).into(), Action::Back));
            return rows;
        }
        let p = self.project();
        let mut rows = vec![
            (
                format!("{}: {}", ai(lang, T::Project), p.name),
                Action::Projects,
            ),
            (ai(lang, T::Rename).into(), Action::Rename),
            (
                format!("{}: {}", ai(lang, T::Tags), p.tags.join(", ")),
                Action::Tags,
            ),
            (
                format!("{}: {}", ai(lang, T::Request), flat(&p.request)),
                Action::Request,
            ),
            (ai(lang, T::Add).into(), Action::Add),
            (ai(lang, T::SaveFavorite).into(), Action::SaveFavorite),
            (ai(lang, T::Review).into(), Action::Review),
        ];
        for (i, key) in variables(&p.request).into_iter().enumerate() {
            rows.push((
                format!(
                    "{} {{{key}}} {}",
                    ai(lang, T::Variable),
                    if p.values.contains_key(&key) {
                        "✓"
                    } else {
                        "…"
                    }
                ),
                Action::Variable(i),
            ));
        }
        for (i, f) in p.favorites.iter().enumerate() {
            rows.push((format!("★ {}", f.name), Action::Favorite(i)));
        }
        for (i, item) in p.items.iter().enumerate() {
            rows.push((
                format!(
                    "{}. [{}] {}{}",
                    i + 1,
                    ai(lang, section_label(item.section)),
                    if item.image {
                        ai(lang, T::Image)
                    } else {
                        &item.text
                    },
                    if item.tags.is_empty() {
                        String::new()
                    } else {
                        format!("  #{}", item.tags.join(" #"))
                    }
                ),
                Action::Item(i),
            ));
        }
        rows.push((ai(lang, T::Clear).into(), Action::Clear));
        rows.push((ai(lang, T::History).into(), Action::Back));
        rows
    }
    pub fn view(&self, lang: Lang) -> View {
        let editor = self
            .project()
            .editor
            .as_ref()
            .filter(|_| !self.choosing_project && self.review.is_none());
        let input = editor.map_or_else(
            || {
                if self.review.is_some() {
                    return ai(lang, T::Warning).into();
                }
                self.notice
                    .map_or_else(|| ai(lang, T::Help).into(), |t| ai(lang, t).into())
            },
            |e| e.text.clone(),
        );
        let hint = editor.map_or_else(
            || ai(lang, T::Keys).into(),
            |e| {
                let label = match &e.kind {
                    EditKind::Project => ai(lang, T::Rename),
                    EditKind::Tags => ai(lang, T::Tags),
                    EditKind::Request => ai(lang, T::Request),
                    EditKind::FavoriteName => ai(lang, T::FavoriteName),
                    EditKind::Variable(key) => {
                        return format!("{{{key}}} • {}", ai(lang, T::EditKeys))
                    }
                };
                format!("{label} • {}", ai(lang, T::EditKeys))
            },
        );
        View {
            editing: editor.is_some(),
            title: format!("{} · {}", ai(lang, T::Title), self.project().name),
            input,
            hint,
            rows: if let Some(e) = editor {
                let lines: Vec<String> = e
                    .text
                    .lines()
                    .flat_map(|l| wrap(l, self.text_width))
                    .collect();
                lines
                    .into_iter()
                    .rev()
                    .take(12)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect()
            } else {
                self.actions(lang)
                    .into_iter()
                    .map(|(s, _)| if self.review.is_some() { s } else { flat(&s) })
                    .collect()
            },
            selected: self.sel,
            scroll: if editor.is_some() { 0 } else { self.scroll },
        }
    }
    pub fn wheel(&mut self, delta: i32, rows: usize, lang: Lang) {
        let total = self.actions(lang).len();
        self.sel = (self.sel as i32 + delta).clamp(0, total.saturating_sub(1) as i32) as usize;
        self.clamp(rows, lang);
    }
    fn clamp(&mut self, rows: usize, lang: Lang) {
        self.sel = self.sel.min(self.actions(lang).len().saturating_sub(1));
        if self.sel < self.scroll {
            self.scroll = self.sel;
        }
        if self.sel >= self.scroll + rows {
            self.scroll = self.sel + 1 - rows;
        }
    }
    pub fn click_row(&mut self, row: usize, lang: Lang, rows: usize) -> Option<String> {
        if self.editing() {
            return None;
        }
        self.sel = row;
        self.nav(NavKey::Enter, lang, rows)
    }
    pub fn nav(&mut self, key: NavKey, lang: Lang, rows: usize) -> Option<String> {
        // A draft remains saved when leaving the workbench or switching project.
        if key == NavKey::Workbench {
            self.toggle();
            return None;
        }
        if self.editing() {
            match key {
                NavKey::Enter => self.finish(),
                NavKey::PasteText => self.paste("\n"),
                NavKey::Esc => {
                    self.project_mut().editor = None;
                    self.dirty = true;
                }
                NavKey::SelectAll => self.select_all = true,
                NavKey::Backspace | NavKey::Delete => {
                    let all = std::mem::take(&mut self.select_all);
                    if let Some(e) = &mut self.project_mut().editor {
                        if all {
                            e.text.clear();
                        } else {
                            e.text.pop();
                        }
                        self.dirty = true;
                    }
                }
                _ => {}
            }
            return None;
        }
        let action = self
            .actions(lang)
            .get(self.sel)
            .map_or(Action::None, |(_, a)| *a);
        match key {
            NavKey::Esc => {
                if self.review.take().is_none() && !std::mem::take(&mut self.choosing_project) {
                    self.open = false;
                }
            }
            NavKey::Up => self.sel = self.sel.saturating_sub(1),
            NavKey::Down => self.sel += 1,
            NavKey::PageDown => self.sel += rows,
            NavKey::PageUp => self.sel = self.sel.saturating_sub(rows),
            NavKey::Home => self.sel = 0,
            NavKey::End => self.sel = self.actions(lang).len().saturating_sub(1),
            NavKey::Delete => match action {
                Action::Item(i) => {
                    self.project_mut().items.remove(i);
                    self.dirty = true;
                }
                Action::Favorite(i) => {
                    self.project_mut().favorites.remove(i);
                    self.dirty = true;
                }
                _ => {}
            },
            NavKey::Left | NavKey::Right => {
                if let Action::Item(i) = action {
                    let item = &mut self.project_mut().items[i];
                    item.section = (item.section + if key == NavKey::Right { 1 } else { 3 }) % 4;
                    self.dirty = true;
                }
            }
            NavKey::Pin | NavKey::Undo => {
                if let Action::Item(i) = action {
                    let j = if key == NavKey::Pin {
                        i.saturating_sub(1)
                    } else {
                        (i + 1).min(self.project().items.len() - 1)
                    };
                    self.project_mut().items.swap(i, j);
                    self.sel = (self.sel as isize + j as isize - i as isize) as usize;
                    self.dirty = true;
                }
            }
            NavKey::Enter => match action {
                Action::Projects => {
                    self.choosing_project = true;
                    self.sel = 0;
                    self.scroll = 0;
                }
                Action::Project(i) => {
                    self.active = i;
                    self.choosing_project = false;
                    self.sel = 0;
                    self.scroll = 0;
                    self.dirty = true;
                }
                Action::NewProject => {
                    if self.projects.len() >= MAX_PROJECTS {
                        self.notice = Some(T::Limit);
                    } else {
                        self.projects.push(Project {
                            name: format!(
                                "{} {}",
                                ai(lang, T::ProjectName),
                                self.projects.len() + 1
                            ),
                            ..Project::default()
                        });
                        self.active = self.projects.len() - 1;
                        self.choosing_project = false;
                        self.sel = 0;
                        self.scroll = 0;
                        self.dirty = true;
                    }
                }
                Action::Rename => self.begin(EditKind::Project, self.project().name.clone()),
                Action::Tags => self.begin(EditKind::Tags, self.project().tags.join(", ")),
                Action::Request => self.begin(EditKind::Request, self.project().request.clone()),
                Action::Add => {
                    self.open = false;
                    self.adding = true;
                }
                Action::SaveFavorite => self.begin(EditKind::FavoriteName, String::new()),
                Action::Favorite(i) => {
                    let p = self.project();
                    let final_bytes = project_bytes(p)
                        - p.request.len()
                        - p.values
                            .iter()
                            .map(|(k, v)| k.len() + v.len())
                            .sum::<usize>()
                        + p.favorites[i].body.len();
                    if final_bytes > MAX_TEXT {
                        self.notice = Some(T::Limit);
                    } else {
                        self.project_mut().request = self.project().favorites[i].body.clone();
                        self.project_mut().values.clear();
                        self.dirty = true;
                    }
                }
                Action::Variable(i) => {
                    if let Some(key) = variables(&self.project().request).get(i) {
                        let val = self.project().values.get(key).cloned().unwrap_or_default();
                        self.begin(EditKind::Variable(key.clone()), val);
                    }
                }
                Action::Review => {
                    if variables(&self.project().request)
                        .iter()
                        .any(|key| !self.project().values.contains_key(key))
                    {
                        self.notice = Some(T::Unfilled);
                    } else {
                        let text = self.compose(lang);
                        if text.is_empty() {
                            self.notice = Some(T::Empty);
                        } else if text.len() > MAX_TEXT {
                            self.notice = Some(T::Limit);
                        } else {
                            self.review = Some(redact(&text, lang));
                            self.sel = 0;
                            self.scroll = 0;
                        }
                    }
                }
                Action::Confirm => {
                    if let Some(review) = self.review.take() {
                        self.notice = Some(T::Copied);
                        return Some(review.text);
                    }
                }
                Action::Back => {
                    if self.review.take().is_none() && !std::mem::take(&mut self.choosing_project) {
                        self.open = false;
                    }
                    self.sel = 0;
                    self.scroll = 0;
                }
                Action::Clear => {
                    self.project_mut().items.clear();
                    self.dirty = true;
                }
                Action::None => {}
                Action::Item(i) => {
                    let item = &mut self.project_mut().items[i];
                    item.section = (item.section + 1) % 4;
                    self.dirty = true;
                }
            },
            _ => {}
        }
        self.clamp(rows, lang);
        None
    }
}
fn flat(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
fn wrap(s: &str, width: f32) -> Vec<String> {
    let mut rows = Vec::new();
    let mut line = String::new();
    for c in s.chars() {
        line.push(c);
        if crate::sysfont::measure(&line, 1.5) > width && line.chars().count() > 1 {
            line.pop();
            rows.push(std::mem::take(&mut line));
            line.push(c);
        }
    }
    rows.push(line);
    rows
}
fn section_label(i: usize) -> T {
    [T::Request, T::Code, T::Errors, T::Constraints][i % 4]
}
pub fn variables(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = s;
    while let Some(start) = rest.find('{') {
        rest = &rest[start + 1..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let key = &rest[..end];
        if !key.is_empty()
            && key.chars().count() <= 40
            && key
                .chars()
                .all(|c| c.is_alphanumeric() || c == ' ' || c == '_' || c == '-')
            && !out.iter().any(|s| s == key)
        {
            out.push(key.into());
        }
        rest = &rest[end + 1..];
    }
    out
}

/// Conservative local pattern matching. Findings contain kind/ordinal only,
/// never a source value. UTF-8 byte ranges are retained only for replacement.
pub fn redact(text: &str, lang: Lang) -> Review {
    let mut ranges: Vec<(usize, usize, T)> = Vec::new();
    // Provider/token prefixes. Require plausible lengths, avoid normal prose.
    for (prefix, min) in [
        ("sk-", 16),
        ("ghp_", 20),
        ("github_pat_", 20),
        ("AKIA", 16),
        ("xoxb-", 16),
        ("xoxp-", 16),
    ] {
        for (start, _) in text.match_indices(prefix) {
            let len = text[start..]
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || b"_-".contains(b))
                .count();
            if len >= min {
                ranges.push((start, start + len, T::Token));
            }
        }
    }
    let mut offset = 0;
    for line in text.split_inclusive('\n') {
        let lower = line.to_ascii_lowercase();
        if let Some(pos) = lower.find("bearer ") {
            let start = pos + 7;
            let len = line[start..]
                .bytes()
                .take_while(|b| b.is_ascii_alphanumeric() || b"._-+/=".contains(b))
                .count();
            if len >= 8 {
                ranges.push((offset + start, offset + start + len, T::Bearer));
            }
        }
        if let Some(sep) = line.find(['=', ':']) {
            let name = lower[..sep].trim().trim_matches(['\"', '\'']);
            if [
                "api_key",
                "apikey",
                "api-key",
                "token",
                "secret",
                "password",
                "passwd",
                "access_key",
                "private_key",
            ]
            .iter()
            .any(|key| name.ends_with(key))
            {
                let value = &line[sep + 1..];
                let leading = value.len() - value.trim_start().len();
                let v = value.trim_start();
                let quote = v.chars().next().filter(|c| *c == '\"' || *c == '\'');
                let skip = usize::from(quote.is_some());
                let raw = &v[skip..];
                let end = if let Some(q) = quote {
                    raw.find(q).unwrap_or(raw.trim_end().len())
                } else {
                    raw.find(|c: char| c.is_whitespace() || c == ',' || c == ';')
                        .unwrap_or(raw.len())
                };
                if end >= 4 {
                    let start = offset + sep + 1 + leading + skip;
                    ranges.push((start, start + end, T::Assignment));
                }
            }
        }
        offset += line.len();
    }
    // PEM private-key blocks must be replaced as one unit, including newlines.
    let mut search = 0;
    while let Some(p) = text[search..].find("-----BEGIN ") {
        let start = search + p;
        let tail = &text[start..];
        if let Some(header_end) = tail.find("-----\n").or_else(|| tail.find("-----\r\n")) {
            if tail[..header_end].contains("PRIVATE KEY") {
                let end = tail
                    .find("-----END ")
                    .and_then(|i| tail[i + 9..].find("-----").map(|j| i + 9 + j + 5))
                    .unwrap_or(tail.len());
                ranges.push((start, start + end, T::PrivateKey));
            }
        }
        search = start + 11;
        if search >= text.len() {
            break;
        }
    }
    ranges.sort_by_key(|r| r.0);
    let mut merged: Vec<(usize, usize, T)> = Vec::new();
    for r in ranges {
        if let Some(last) = merged.last_mut() {
            if r.0 < last.1 {
                last.1 = last.1.max(r.1);
                continue;
            }
        }
        merged.push(r);
    }
    let mut out = String::new();
    let mut cursor = 0;
    let mut findings = Vec::new();
    for (i, (start, end, kind)) in merged.iter().enumerate() {
        out.push_str(&text[cursor..*start]);
        let label = format!("[REDACTED_{}]", i + 1);
        out.push_str(&label);
        findings.push(format!(
            "{} {} → {}",
            ai(lang, T::Sensitive),
            ai(lang, *kind),
            label
        ));
        cursor = *end;
    }
    out.push_str(&text[cursor..]);
    Review {
        text: out,
        findings,
    }
}

/// Expand only placeholders in the original template. Values are literal text.
fn expanded(template: &str, values: &BTreeMap<String, String>) -> Option<String> {
    let keys = variables(template);
    let mut out = String::new();
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        let Some(end) = rest.find('}') else {
            break;
        };
        let key = &rest[1..end];
        if keys.iter().any(|k| k == key) {
            out.push_str(values.get(key).map_or(&rest[..=end], String::as_str));
        } else {
            out.push_str(&rest[..=end]);
        }
        rest = &rest[end + 1..];
        if out.len() > MAX_DRAFT {
            return None;
        }
    }
    out.push_str(rest);
    (out.len() <= MAX_DRAFT).then_some(out)
}

fn project_bytes(p: &Project) -> usize {
    p.name.len()
        + p.request.len()
        + p.tags.iter().map(String::len).sum::<usize>()
        + p.items
            .iter()
            .map(|i| i.text.len() + i.tags.iter().map(String::len).sum::<usize>())
            .sum::<usize>()
        + p.favorites
            .iter()
            .map(|f| f.name.len() + f.body.len())
            .sum::<usize>()
        + p.values
            .iter()
            .map(|(k, v)| k.len() + v.len())
            .sum::<usize>()
        + p.editor.as_ref().map_or(0, |e| {
            e.text.len()
                + match &e.kind {
                    EditKind::Variable(k) => k.len(),
                    _ => 0,
                }
        })
}

fn valid_tags(tags: &[String]) -> bool {
    parse_tags(&tags.join(",")).as_deref() == Some(tags)
}

#[cfg(test)]
#[path = "ai_workbench_tests.rs"]
mod tests;
