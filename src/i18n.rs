//! UI strings in English and Korean. Every user-visible string lives here so
//! both backends (tray menu, panel, toasts, bubble) stay in sync across
//! languages. The language is persisted in [`crate::state::Persist::lang`];
//! the first run picks a default from the OS locale (`state::detect_lang`).

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    En,
    Ko,
}

impl Lang {
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ko => "ko",
        }
    }

    pub fn from_code(code: &str) -> Option<Lang> {
        match code {
            "en" => Some(Lang::En),
            "ko" => Some(Lang::Ko),
            _ => None,
        }
    }

    pub fn toggled(self) -> Lang {
        match self {
            Lang::En => Lang::Ko,
            Lang::Ko => Lang::En,
        }
    }
}

/// Every fixed UI string. Formatted strings get helper fns below.
#[derive(Clone, Copy)]
pub enum Msg {
    // tray menu (native backend)
    MenuShowStats,
    MenuCapturePause,
    MenuSize,
    SizeSmall,
    SizeNormal,
    SizeLarge,
    MenuAccessory,
    AccNone,
    AccRedScarf,
    AccGlasses,
    AccBlueBeanie,
    AccHeadphones,
    AccGoldCrown,
    AccWizardHat,
    AccBunnyEars,
    AccSprout,
    AccDaisyCrown,
    AccBearEars,
    AccCherry,
    AccButterfly,
    AccHeartShades,
    AccChick,
    AccSleepMask,
    AccNightcap,
    AccFishHat,
    AccFishBread,
    AccLuckyClover,
    AccPudding,
    MenuSound,
    SoundOff,
    SoundEvents,
    SoundAll,
    MenuWindowLevel,
    WinLevelTop,
    WinLevelNormal,
    WinLevelHide,
    MenuLock,
    MenuLanguage,
    MenuAutostart,
    MenuLoginStart,
    MenuReset,
    Cancel,
    MenuAbout,
    MenuGithub,
    MenuExit,
    ResetTitle,
    ResetConfirm,
    // clipboard panel
    PanelTitle,
    SearchHint,
    PanelEmpty,
    PanelNoMatch,
    FooterKeys,
    MenuAutoClose,
    MenuPasteOnSelect,
    ToastCopied,
    ToastPasteOn,
    ToastPasteOff,
    ToastDeleted,
    ToastRestored,
    ToastClearConfirm,
    ToastCapturePaused,
    ToastCaptureOn,
    ToastAutoCloseOn,
    ToastAutoCloseOff,
    /// Toast shown by the per-row "paste as text" action (ADR-0014).
    ToastPastedPlain,
    // panel header-icon tooltips
    TipView,
    TipFilter,
    TipPause,
    TipResume,
    TipClear,
    TipLang,
    TipClose,
    TipPin,
    TipUnpin,
    // auto-update (ADR-0009)
    MenuAutoUpdate,
    ToastUpdateDownloading,
    ToastUpdateFailed,
    // macOS Accessibility permission (global input tap)
    ToastAccessibility,
    // stats bubble
    BubbleLv,
    BubbleKeys,
    BubbleClicks,
    BubbleClips,
    BubbleActive,
}

/// Looks up a fixed string for the language.
pub fn t(lang: Lang, msg: Msg) -> &'static str {
    use Msg::*;
    match (msg, lang) {
        (MenuShowStats, Lang::En) => "Always show stats",
        (MenuShowStats, Lang::Ko) => "통계 항상 표시",
        (MenuCapturePause, Lang::En) => "Pause clip capture",
        (MenuCapturePause, Lang::Ko) => "클립 수집 일시정지",
        (MenuSize, Lang::En) => "Size",
        (MenuSize, Lang::Ko) => "크기",
        (SizeSmall, Lang::En) => "Small",
        (SizeSmall, Lang::Ko) => "작게",
        (SizeNormal, Lang::En) => "Normal",
        (SizeNormal, Lang::Ko) => "보통",
        (SizeLarge, Lang::En) => "Large",
        (SizeLarge, Lang::Ko) => "크게",
        (MenuAccessory, Lang::En) => "Accessory",
        (MenuAccessory, Lang::Ko) => "액세서리",
        (AccNone, Lang::En) => "None",
        (AccNone, Lang::Ko) => "없음",
        (AccRedScarf, Lang::En) => "RED SCARF",
        (AccRedScarf, Lang::Ko) => "빨간 목도리",
        (AccGlasses, Lang::En) => "GLASSES",
        (AccGlasses, Lang::Ko) => "동그란 안경",
        (AccBlueBeanie, Lang::En) => "BLUE BEANIE",
        (AccBlueBeanie, Lang::Ko) => "파란 비니",
        (AccHeadphones, Lang::En) => "HEADPHONES",
        (AccHeadphones, Lang::Ko) => "헤드폰",
        (AccGoldCrown, Lang::En) => "GOLD CROWN",
        (AccGoldCrown, Lang::Ko) => "황금 왕관",
        (AccWizardHat, Lang::En) => "WIZARD HAT",
        (AccWizardHat, Lang::Ko) => "마법사 모자",
        (AccBunnyEars, Lang::En) => "BUNNY EARS",
        (AccBunnyEars, Lang::Ko) => "토끼 귀",
        (AccSprout, Lang::En) => "SPROUT",
        (AccSprout, Lang::Ko) => "새싹",
        (AccDaisyCrown, Lang::En) => "DAISY CROWN",
        (AccDaisyCrown, Lang::Ko) => "데이지 화관",
        (AccBearEars, Lang::En) => "BEAR EARS",
        (AccBearEars, Lang::Ko) => "곰 귀",
        (AccCherry, Lang::En) => "CHERRY",
        (AccCherry, Lang::Ko) => "체리",
        (AccButterfly, Lang::En) => "BUTTERFLY",
        (AccButterfly, Lang::Ko) => "나비",
        (AccHeartShades, Lang::En) => "HEART SHADES",
        (AccHeartShades, Lang::Ko) => "하트 선글라스",
        (AccChick, Lang::En) => "CHICK",
        (AccChick, Lang::Ko) => "병아리",
        (AccSleepMask, Lang::En) => "SLEEP MASK",
        (AccSleepMask, Lang::Ko) => "수면 안대",
        (AccNightcap, Lang::En) => "NIGHTCAP",
        (AccNightcap, Lang::Ko) => "수면 모자",
        (AccFishHat, Lang::En) => "FISH HAT",
        (AccFishHat, Lang::Ko) => "생선 모자",
        (AccFishBread, Lang::En) => "FISH BREAD",
        (AccFishBread, Lang::Ko) => "붕어빵",
        (AccLuckyClover, Lang::En) => "LUCKY CLOVER",
        (AccLuckyClover, Lang::Ko) => "네잎클로버",
        (AccPudding, Lang::En) => "PUDDING",
        (AccPudding, Lang::Ko) => "푸딩",
        (MenuSound, Lang::En) => "Sound",
        (MenuSound, Lang::Ko) => "소리",
        (SoundOff, Lang::En) => "Off",
        (SoundOff, Lang::Ko) => "끄기",
        (SoundEvents, Lang::En) => "Events only",
        (SoundEvents, Lang::Ko) => "이벤트 소리만",
        (SoundAll, Lang::En) => "Typing + events",
        (SoundAll, Lang::Ko) => "타이핑 소리 + 이벤트",
        (MenuWindowLevel, Lang::En) => "Window",
        (MenuWindowLevel, Lang::Ko) => "창",
        (WinLevelTop, Lang::En) => "Always on top",
        (WinLevelTop, Lang::Ko) => "항상 위",
        (WinLevelNormal, Lang::En) => "Normal",
        (WinLevelNormal, Lang::Ko) => "보통",
        (WinLevelHide, Lang::En) => "Hide",
        (WinLevelHide, Lang::Ko) => "숨기기",
        (MenuLock, Lang::En) => "Lock position",
        (MenuLock, Lang::Ko) => "위치 잠금",
        (MenuLanguage, _) => "Language (언어)",
        (MenuAutostart, Lang::En) => "Run at Windows startup",
        (MenuAutostart, Lang::Ko) => "Windows 시작 시 자동 실행",
        (MenuLoginStart, Lang::En) => "Run at login",
        (MenuLoginStart, Lang::Ko) => "로그인 시 자동 실행",
        (MenuReset, Lang::En) => "Reset stats...",
        (MenuReset, Lang::Ko) => "통계 초기화...",
        (Cancel, Lang::En) => "Cancel",
        (Cancel, Lang::Ko) => "취소",
        (MenuAbout, Lang::En) => "About ClipCat",
        (MenuAbout, Lang::Ko) => "ClipCat 정보",
        (MenuGithub, _) => "GitHub",
        (MenuExit, Lang::En) => "Quit",
        (MenuExit, Lang::Ko) => "종료",
        (ResetTitle, Lang::En) => "Reset stats",
        (ResetTitle, Lang::Ko) => "통계 초기화",
        (ResetConfirm, Lang::En) => {
            "Reset all stats and the level?\n(This cannot be undone. Clips are kept.)"
        }
        (ResetConfirm, Lang::Ko) => {
            "모든 통계와 레벨을 초기화할까요?\n(되돌릴 수 없습니다. 클립은 유지됩니다.)"
        }
        (PanelTitle, Lang::En) => "CLIPBOARD",
        (PanelTitle, Lang::Ko) => "클립보드",
        (SearchHint, Lang::En) => "Search...",
        (SearchHint, Lang::Ko) => "검색...",
        (PanelEmpty, Lang::En) => "Copy something - it lands here!",
        (PanelEmpty, Lang::Ko) => "복사하면 여기에 쌓여요!",
        (PanelNoMatch, Lang::En) => "No matching clips",
        (PanelNoMatch, Lang::Ko) => "검색 결과가 없어요",
        (FooterKeys, Lang::En) => "ENTER COPY | ^ENTER TEXT | ^0-9 QUICK | DEL DELETE | ^Z UNDO | ^P PIN | TAB APP",
        (FooterKeys, Lang::Ko) => "Enter 복사 | ^Enter 텍스트 | ^0-9 바로 복사 | Del 삭제 | ^Z 복구 | ^P 고정 | Tab 앱별",
        (MenuAutoClose, Lang::En) => "Close panel after copy",
        (MenuAutoClose, Lang::Ko) => "복사 후 패널 자동 닫기",
        (MenuPasteOnSelect, Lang::En) => "Paste on select",
        (MenuPasteOnSelect, Lang::Ko) => "선택 시 바로 붙여넣기",
        (ToastCopied, Lang::En) => "COPIED!",
        (ToastCopied, Lang::Ko) => "복사됨!",
        (ToastPasteOn, Lang::En) => "AUTO-PASTE ON",
        (ToastPasteOn, Lang::Ko) => "선택 시 붙여넣기 켜짐",
        (ToastPasteOff, Lang::En) => "AUTO-PASTE OFF",
        (ToastPasteOff, Lang::Ko) => "선택 시 붙여넣기 꺼짐",
        (ToastDeleted, Lang::En) => "DELETED - CTRL+Z TO UNDO",
        (ToastDeleted, Lang::Ko) => "삭제됨 - Ctrl+Z로 복구",
        (ToastRestored, Lang::En) => "RESTORED!",
        (ToastRestored, Lang::Ko) => "복구됨!",
        (ToastClearConfirm, Lang::En) => "PRESS AGAIN TO CLEAR ALL",
        (ToastClearConfirm, Lang::Ko) => "한 번 더 누르면 모두 삭제돼요",
        (ToastCapturePaused, Lang::En) => "CAPTURE PAUSED",
        (ToastCapturePaused, Lang::Ko) => "클립 수집 일시정지",
        (ToastCaptureOn, Lang::En) => "CAPTURE ON",
        (ToastCaptureOn, Lang::Ko) => "클립 수집 재개",
        (ToastAutoCloseOn, Lang::En) => "PANEL CLOSES AFTER COPY",
        (ToastAutoCloseOn, Lang::Ko) => "복사 후 패널을 닫아요",
        (ToastAutoCloseOff, Lang::En) => "PANEL STAYS OPEN AFTER COPY",
        (ToastAutoCloseOff, Lang::Ko) => "복사 후에도 패널이 열려 있어요",
        (ToastPastedPlain, Lang::En) => "PASTED AS TEXT",
        (ToastPastedPlain, Lang::Ko) => "텍스트로 붙여넣었어요",
        (TipView, Lang::En) => "List / card view",
        (TipView, Lang::Ko) => "목록 / 카드 보기",
        (TipFilter, Lang::En) => "Filter by app",
        (TipFilter, Lang::Ko) => "앱별 필터",
        (TipPause, Lang::En) => "Pause capture",
        (TipPause, Lang::Ko) => "수집 일시정지",
        (TipResume, Lang::En) => "Resume capture",
        (TipResume, Lang::Ko) => "수집 다시 시작",
        (TipClear, Lang::En) => "Clear history",
        (TipClear, Lang::Ko) => "기록 비우기",
        (TipLang, Lang::En) => "Language",
        (TipLang, Lang::Ko) => "언어",
        (TipClose, Lang::En) => "Close",
        (TipClose, Lang::Ko) => "닫기",
        (TipPin, Lang::En) => "Pin (Ctrl/Cmd+P)",
        (TipPin, Lang::Ko) => "고정 (Ctrl/Cmd+P)",
        (TipUnpin, Lang::En) => "Unpin (Ctrl/Cmd+P)",
        (TipUnpin, Lang::Ko) => "고정 해제 (Ctrl/Cmd+P)",
        (MenuAutoUpdate, Lang::En) => "Check for updates automatically",
        (MenuAutoUpdate, Lang::Ko) => "자동 업데이트 확인",
        (ToastUpdateDownloading, Lang::En) => "DOWNLOADING UPDATE...",
        (ToastUpdateDownloading, Lang::Ko) => "업데이트 다운로드 중...",
        (ToastUpdateFailed, Lang::En) => "UPDATE FAILED",
        (ToastUpdateFailed, Lang::Ko) => "업데이트 실패",
        (ToastAccessibility, Lang::En) => "ENABLE ACCESSIBILITY IN SYSTEM SETTINGS FOR THE HOTKEY",
        (ToastAccessibility, Lang::Ko) => "단축키 사용: 시스템 설정 > 손쉬운 사용에서 권한을 허용하세요",
        (BubbleLv, Lang::En) => "LV",
        (BubbleLv, Lang::Ko) => "LV",
        (BubbleKeys, Lang::En) => "KEYS",
        (BubbleKeys, Lang::Ko) => "키 입력",
        (BubbleClicks, Lang::En) => "CLICKS",
        (BubbleClicks, Lang::Ko) => "클릭",
        (BubbleClips, Lang::En) => "CLIPS",
        (BubbleClips, Lang::Ko) => "복사",
        (BubbleActive, Lang::En) => "ACTIVE",
        (BubbleActive, Lang::Ko) => "활동",
    }
}

// ---- formatted strings ------------------------------------------------------

/// Tray-menu entry for the clipboard panel, with the live hotkey label
/// (e.g. "WIN+SHIFT+V") as the Windows menu accelerator column.
pub fn menu_clipboard(lang: Lang, hotkey: &str) -> String {
    match lang {
        Lang::En => format!("Clipboard history\t{hotkey}"),
        Lang::Ko => format!("클립보드 히스토리\t{hotkey}"),
    }
}

/// Settings-menu entry for the panel hotkey: shows the live chord; choosing it
/// cycles to the next preset (see [`crate::hotkey::next_preset`]).
pub fn menu_hotkey(lang: Lang, hotkey: &str) -> String {
    match lang {
        Lang::En => format!("Panel hotkey: {hotkey}"),
        Lang::Ko => format!("패널 단축키: {hotkey}"),
    }
}

/// First-run hint shown by the pet (until the panel is first opened): the live
/// hotkey chord that opens the clipboard history.
pub fn first_run_hint(lang: Lang, hotkey: &str) -> String {
    match lang {
        Lang::En => format!("Clipboard: {hotkey}"),
        Lang::Ko => format!("클립보드: {hotkey}"),
    }
}

pub fn level_up(lang: Lang, lv: u32) -> String {
    match lang {
        Lang::En => format!("LEVEL UP! LV {lv}"),
        Lang::Ko => format!("레벨 업! LV {lv}"),
    }
}

/// The stats-bubble "active time" value (hours + minutes), localized so the
/// time units read natively instead of a hardcoded English "H"/"M".
pub fn fmt_active(lang: Lang, h: u32, m: u32) -> String {
    match lang {
        Lang::En => format!("{h}H {m:02}M"),
        Lang::Ko => format!("{h}시간 {m:02}분"),
    }
}

pub fn new_accessory(lang: Lang, name: &str) -> String {
    match lang {
        Lang::En => format!("* NEW: {name} *"),
        Lang::Ko => format!("* 새 아이템: {name} *"),
    }
}

pub fn accessory_locked(lang: Lang, name: &str, level: u32) -> String {
    match lang {
        Lang::En => format!("{name} (unlocks at LV {level})"),
        Lang::Ko => format!("{name} (LV {level} 달성 시)"),
    }
}

/// Toast when the background check finds a newer release.
pub fn update_available(lang: Lang, ver: &str) -> String {
    match lang {
        Lang::En => format!("NEW VERSION v{ver}!"),
        Lang::Ko => format!("새 버전 v{ver}!"),
    }
}

/// Tray-menu entry that downloads the found update and restarts (Windows).
pub fn menu_update(lang: Lang, ver: &str) -> String {
    match lang {
        Lang::En => format!("Update to v{ver} and restart"),
        Lang::Ko => format!("v{ver} 업데이트 후 재시작"),
    }
}

pub fn cleared_clips(lang: Lang, n: usize) -> String {
    match lang {
        Lang::En => format!("CLEARED {n} CLIPS"),
        Lang::Ko => format!("클립 {n}개 지움"),
    }
}

/// Toast shown when the configured panel hotkey could not be registered (e.g.
/// Windows reserves Win+Shift+V for clipboard history) and the fallback chord
/// took over — so the displayed label stops looking like a silent mismatch
/// with the saved setting.
pub fn hotkey_fallback(lang: Lang, wanted: &str, used: &str) -> String {
    match lang {
        Lang::En => {
            format!("{wanted} unavailable — using {used} (clipboard history or another app owns it)")
        }
        Lang::Ko => format!(
            "{wanted} 단축키를 사용할 수 없어 {used}로 대체했습니다 (클립보드 기록 또는 다른 앱이 사용 중)"
        ),
    }
}

/// Footer line of the panel: clip count and pinned count.
pub fn clip_count(lang: Lang, total: usize, pinned: usize) -> String {
    match lang {
        Lang::En => format!("{total} CLIPS - {pinned} PINNED"),
        Lang::Ko => format!("클립 {total}개 - 고정 {pinned}개"),
    }
}

/// Short relative timestamp for clip rows ("now", "5m", "2h", "3d").
pub fn time_ago(lang: Lang, secs: u64) -> String {
    let (n, unit_en, unit_ko) = if secs < 60 {
        return match lang {
            Lang::En => "NOW".to_string(),
            Lang::Ko => "방금".to_string(),
        };
    } else if secs < 3600 {
        (secs / 60, "M", "분")
    } else if secs < 86_400 {
        (secs / 3600, "H", "시간")
    } else {
        (secs / 86_400, "D", "일")
    };
    match lang {
        Lang::En => format!("{n}{unit_en}"),
        Lang::Ko => format!("{n}{unit_ko}"),
    }
}

/// Body of the native About dialog. `hotkey` is the live panel hotkey label.
pub fn about_text(
    lang: Lang,
    version: &str,
    hotkey: &str,
    lv: u32,
    keys: u64,
    clips: usize,
) -> String {
    match lang {
        Lang::En => format!(
            "ClipCat v{version}\n\nA desktop cat that manages your clipboard \
             and grows with your typing.\n\nLevel: LV {lv}\nLifetime keys: {keys}\nStored \
             clips: {clips}\n\n- Copy anywhere: the cat eats a fish and saves the clip\n\
             - {hotkey} or middle-click: clipboard history\n- Click a clip: paste it \
             back with its original formatting (or use \"...\" to paste as plain text)\n\
             - Type/click: the cat taps \
             along and earns XP\n- Double-click: pet the \
             cat\n\nEverything stays on this PC; the only network use is an \
             optional daily GitHub check for new versions (toggle in this menu)."
        ),
        Lang::Ko => format!(
            "ClipCat v{version}\n\n클립보드를 관리하고 타이핑과 함께 자라는 데스크탑 \
             고양이.\n\n현재 레벨: LV {lv}\n누적 키 입력: {keys}\n저장된 클립: {clips}개\n\n\
             - 어디서든 복사 → 고양이가 생선을 먹고 클립을 저장\n- {hotkey} 또는 \
             휠클릭 → 클립보드 히스토리\n- 클립 클릭 → 원본 서식 그대로 붙여넣기 \
             (\"...\"로 텍스트만 붙여넣기 가능)\n- 타이핑/클릭 → \
             고양이가 따라 치고 XP 획득\n- 더블클릭 → 쓰다듬기\n\n모든 데이터는 이 PC에만 \
             저장됩니다. 네트워크는 GitHub 새 버전 확인에만 쓰입니다(이 메뉴에서 끌 수 \
             있음)."
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_codes_round_trip() {
        for lang in [Lang::En, Lang::Ko] {
            assert_eq!(Lang::from_code(lang.code()), Some(lang));
        }
        assert_eq!(Lang::from_code("fr"), None);
    }

    #[test]
    fn every_message_has_both_translations() {
        use Msg::*;
        let all = [
            MenuShowStats, MenuCapturePause, MenuSize, SizeSmall, SizeNormal,
            SizeLarge, MenuAccessory, AccNone,
            AccRedScarf, AccGlasses, AccBlueBeanie, AccHeadphones, AccGoldCrown, AccWizardHat,
            AccBunnyEars, AccSprout, AccDaisyCrown, AccBearEars, AccCherry, AccButterfly,
            AccHeartShades, AccChick, AccSleepMask, AccNightcap, AccFishHat, AccFishBread,
            AccLuckyClover, AccPudding,
            MenuSound, SoundOff, SoundEvents, SoundAll,
            MenuWindowLevel, WinLevelTop, WinLevelNormal, WinLevelHide,
            MenuLock, MenuLanguage, MenuAutostart, MenuLoginStart, MenuReset, Cancel, MenuAbout,
            MenuGithub, MenuExit, ResetTitle,
            ResetConfirm, PanelTitle, SearchHint, PanelEmpty, PanelNoMatch, FooterKeys,
            MenuAutoClose, MenuPasteOnSelect, ToastCopied, ToastPasteOn, ToastPasteOff,
            ToastDeleted, ToastRestored, ToastClearConfirm,
            ToastCapturePaused, ToastCaptureOn, ToastAutoCloseOn, ToastAutoCloseOff,
            ToastPastedPlain, TipView, TipFilter, TipPause, TipResume, TipClear, TipLang, TipClose,
            TipPin, TipUnpin,
            MenuAutoUpdate, ToastUpdateDownloading,
            ToastUpdateFailed, ToastAccessibility, BubbleLv, BubbleKeys, BubbleClicks, BubbleClips,
            BubbleActive,
        ];
        for msg in all {
            assert!(!t(Lang::En, msg).is_empty());
            assert!(!t(Lang::Ko, msg).is_empty());
        }
        for lang in [Lang::En, Lang::Ko] {
            assert!(menu_clipboard(lang, "WIN+SHIFT+V").contains("WIN+SHIFT+V"));
            assert!(about_text(lang, "2.1.0", "WIN+SHIFT+V", 3, 10, 4).contains("WIN+SHIFT+V"));
            assert!(update_available(lang, "2.1.0").contains("v2.1.0"));
            assert!(menu_update(lang, "2.1.0").contains("v2.1.0"));
            let fb = hotkey_fallback(lang, "WIN+SHIFT+V", "CTRL+SHIFT+V");
            assert!(fb.contains("WIN+SHIFT+V") && fb.contains("CTRL+SHIFT+V"));
        }
    }

    #[test]
    fn time_ago_buckets() {
        assert_eq!(time_ago(Lang::En, 5), "NOW");
        assert_eq!(time_ago(Lang::En, 90), "1M");
        assert_eq!(time_ago(Lang::Ko, 7200), "2시간");
        assert_eq!(time_ago(Lang::En, 200_000), "2D");
    }
}
