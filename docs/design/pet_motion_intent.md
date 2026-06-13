# Pet Motion Intent

## Visual Identity

ClipCat is a soft, premium, friendly desktop cat.

Core look:

- White/cream fur
- Rounded face and body
- Small black eyes
- Soft pink nose
- Red scarf as signature accessory
- Chunky paws
- Expressive open mouth for copy/nom/yawn
- Gentle highlight and shadow, suitable for dark UI
- No sharp edges
- No scary expression
- No overly realistic animal detail

## Emotional Range

The pet should communicate productivity feedback without interrupting work.

States:

- Idle: calm, alive, blinking
- Typing slow: focused and lightly tapping
- Typing fast: excited, more active tapping
- Typing extreme: thrilled, sparkle/fire energy
- Sleep: curled up, Zzz bubble
- Yawn: open mouth, sleepy
- Look around: curious glance
- Copy reaction: surprised/open mouth, eats fish, happy
- Petting: eyes closed, hearts, purr
- Boop: surprised, tiny star
- Level up: jumping with star burst
- New item: proud, accessory highlighted

## Motion Personality

- Fast feedback for user input.
- Idle animation should be subtle.
- Copy event should feel rewarding.
- Level-up should feel rare and celebratory.
- Petting and boop should feel optional and delightful.
- Hover bubble should feel like an RPG status card.

## Rendering Guidance

Preferred runtime asset model:

- Static atlas or SVG/PNG parts per state.
- Transform-based animation for paws, head, body, fish, bubbles, and FX.
- Do not render each frame as a huge bitmap if memory is constrained.
- Use pre-rendered PNG previews for QA if needed.
