# 07. Component Inventory

## Pet components
- CatBody: base cream shape, ears, eyes, nose, mouth.
- PawLeft/PawRight: independent transform for typing.
- Keyboard: shown only in typing states.
- Scarf: default or Lv2 accessory.
- AccessoryLayer: one active cosmetic at a time unless future stacking is enabled.
- Fish: body, fin, eye, source badge, optional +N queue count.
- FX: sparkle, heart, XP popup, level burst, Zzz.

## Panel components
- PanelShell
- HeaderButton
- SourceFilterButton
- SearchBox
- SourceFilterChip
- ClipRow
- PinStar
- QuickCopyBadge
- SourceBadge
- DeleteButton
- Scrollbar
- FooterHint
- ResizeGrip
- Toast

## States per component
Button: default, hover, pressed, disabled, active, armed-danger.
ClipRow: default, hover, selected, selected-hover, deleting, pinned.
SearchBox: empty, typing, composing, filtered, focused.
Toast: info, success, warning, danger.
PanelShell: default, dragging, resizing.
Pet: idle, typing, sleeping, nom, happy, levelup.
