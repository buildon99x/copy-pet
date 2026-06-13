# 09. Required Preview Frames

Create or update `cargo run --release --example preview` to export these PNG frames:

1. `frame_01_pet_idle.png`
2. `frame_02_pet_typing_slow.png`
3. `frame_03_pet_typing_extreme.png`
4. `frame_04_copy_fish_midflight.png`
5. `frame_05_nom_xp.png`
6. `frame_06_hover_stats_bubble.png`
7. `frame_07_panel_default.png`
8. `frame_08_panel_search_korean.png`
9. `frame_09_panel_source_filter.png`
10. `frame_10_clear_armed_danger.png`
11. `frame_11_empty_state.png`
12. `frame_12_permission_missing.png`

Pass criteria:
- All text legible at 1x and 2x.
- Cat anchor does not move between panel open and closed frames.
- Gold selection border appears only on active/selected item.
- Korean text renders through OS font fallback.
- No clipped rounded corners or invalid transparent pixels.
