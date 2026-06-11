# DeskCat — Product & Technical Spec

Status: implemented (v1.0) · Owner: DeskCat · Last updated: 2026-06-12

## 1. Summary

DeskCat is a desktop companion: a small cat that sits at the bottom of your
screen and "types along" with you (à la Bongo Cat), gaining XP from your
keyboard/mouse activity, leveling up, and unlocking cosmetic accessories. It
doubles as a lightweight activity tracker (keys / clicks / active minutes
today). It is intentionally tiny, dependency-light and frameless.

References that shaped the design: **Bongo Cat** (reactive paw-tapping mascot)
and **Taskbar Hero** (passive productivity meter that lives on the taskbar).

## 2. Goals / Non-goals

**Goals**
- A genuine core loop: input → reaction → XP → level → unlock → repeat.
- Visually charming, immediately legible, zero configuration to start.
- End-to-end and "ships": single binary, persistence, tray/menu, autostart,
  icon/version metadata, graceful shutdown.
- Cross-platform: Windows (primary, premium native experience), macOS, Linux.
- Privacy-preserving: never records *which* keys are pressed.

**Non-goals**
- Not a full pet-sim (no feeding/hunger/decay). Engagement comes from real work.
- Not a keylogger or analytics product; no network, no telemetry.
- Not a general widget framework.

## 3. Core loop

1. The user types or clicks anywhere on the system.
2. Global hooks count events (never their content).
3. Each ~33 ms tick the pet consumes the counts: paws tap (alternating), an
   "excitement" value rises with input rate (fast typing → sweat drop), and XP
   is granted: **2 XP / key, 1 XP / click, 1 XP / scroll**.
4. Crossing an XP threshold levels the pet up: star burst + chime, tray tooltip
   updates, and at specific levels an accessory unlocks and auto-equips.
5. Idle ≥ 75 s → the cat falls asleep (Zzz); any input wakes it (sparkles).

Progression math (`state::xp_to_next`): `200 + 80 · level²` XP per level,
clamped at level 99.

| Level | Unlock |
|------:|--------|
| 2 | Red scarf |
| 3 | Round glasses |
| 5 | Blue beanie |
| 7 | Headphones |
| 10 | Gold crown |
| 15 | Wizard hat |

## 4. Interactions

| Gesture | Effect |
|---------|--------|
| Drag | Move the pet (unless position-locked) |
| Single click | Squash bounce + sparkle (+1 XP) |
| Double click | Pet it: heart burst (+10 XP) |
| Hover | Show today's stats bubble (level/XP bar, keys, clicks, active time) |
| Right click (native) | Context menu: size, accessory, sound, lock, autostart, reset, about, quit |

## 5. Platforms & backends

Two backends share one platform-agnostic core (`pet::Pet`, `render`, `state`,
`font`, `sound`). Exactly one is compiled per build (see ADR-0001).

- **Windows (native, default):** Win32 layered window with **per-pixel alpha
  and click-through**, low-level `WH_*_LL` input hooks, Shell tray icon with a
  full Korean context menu, HKCU autostart. This is the release target.
- **macOS / Linux (portable):** `winit` window + `softbuffer` presentation +
  `rdev` global input. The pet sits on an opaque rounded "card" (softbuffer has
  no per-pixel desktop alpha — ADR-0003). Settings via keyboard shortcuts
  (ADR-0004): `S` size · `A` accessory · `M` sound · `B` bubble · `L` lock ·
  `Q`/`Esc` quit. The portable backend can also be run on Windows with
  `--features portable` for development.

Platform caveats (documented for users): global input requires Accessibility
permission on macOS and an X11 session on Linux (Wayland blocks global capture);
audio is currently Windows-only (ADR-0002).

## 6. Persistence

State is JSON at the per-OS config dir (`state::state_dir`):
Windows `%APPDATA%\DeskCat`, macOS `~/Library/Application Support/DeskCat`,
Linux `$XDG_CONFIG_HOME/DeskCat`. It holds lifetime totals, today's counters
(reset on local-date rollover), window position and all settings. Saved on a
30 s throttle when dirty, on size change, on drag-end and at shutdown.

## 7. Privacy

The input hooks only ever increment three atomic counters
(`input::{KEYS,CLICKS,WHEEL}`). No keycodes, characters, window titles or
timing are stored or transmitted. There is no network code in the binary.

## 8. Quality bar

- Single binary, no installer, no bundled assets (icon, font and sounds are all
  generated from code).
- Idle/active CPU ≈ a few percent of one core (release); memory ~12–16 MB.
- `cargo clippy` clean; `cargo test` green; CI builds on all three OSes.
