---
status: draft
crate: amigo_td
depends_on: ["engine/ui", "games/td/design"]
last_updated: 2026-03-16
---

# Amigo TD -- UI/UX Design Specification

## Purpose

Complete UI/UX design for the Amigo TD tower defense game. All UI renders through the engine's sprite batcher at virtual resolution (640x360 base). No egui, no HTML overlays. Every button, panel, and icon is pixel art that matches the current world's aesthetic.

## Public API

Game-level UI -- no public API. Consumes [engine/ui](../../engine/ui.md) Pixel UI system.

## Behavior

See the full specification below covering all 21 sections: screen flow, HUD, radial menus, world-themed frames, gamepad/keyboard controls, accessibility, animations, and asset requirements.

## Internal Design

Immediate-mode Pixel UI (Tier 1) from the engine, with world-themed 9-slice sprite frames swapped per world. Radial menus for contextual tower interactions. All UI elements are sprite-based.

## Non-Goals

- HTML/CSS overlays
- egui integration
- Complex layout engine (CSS-like)
- Drag-and-drop tower placement (radial menu is the pattern)

## Open Questions

- Exact tower count per world (affects radial menu layout if >4)
- Tutorial/onboarding UI screens
- Multiplayer lobby UI

---

## 1. Design Philosophy

**Pixel-native, world-themed, minimal.**

All UI renders through the sprite batcher at virtual resolution (640x360 base). No egui, no HTML overlays. Every button, panel, and icon is pixel art that matches the current world's aesthetic. The HUD stays out of the way -- the map is the star. Inspired by Kingdom Rush's radial menus (contextual, attached to selection) over Bloons TD's sidebar panels (always visible, eats screen space).

### Principles

- **Context over chrome** -- show information where the player is looking, not in a corner
- **World-themed** -- UI frames, colors, and icons adapt per world (pirate wood panels for Caribbean, stone for LotR, neon for Matrix)
- **Readable at a glance** -- bitmap font, high contrast, consistent icon language
- **Gamepad-friendly** -- every interaction works with D-pad + face buttons, cursor snaps to grid
- **Minimal clicks** -- tower place = 2 clicks (empty tile -> tower icon), upgrade = 2 clicks (tower -> upgrade icon)

---

## 2. Virtual Resolution & Safe Zones

```
640 x 360 virtual pixels (16:9, scales to any resolution)

+------------------------------------------------------------+
| [HUD BAR - 640 x 16px]                                     |  <- Top bar
|  ♥ 20  |  Wave 3/10  |  Gold 450  |  Pause Speed Settings  |
+------------------------------------------------------------+
|                                                              |
|                                                              |
|                    GAME MAP                                  |
|                    598 x 344 px                               |
|                    (full playfield)                           |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
|                                                              |
+------------------------------------------------------------+
```

Top bar is the only permanent UI. Everything else is contextual (appears on interaction, fades when not needed).

---

## 3. Screen Flow

```
+----------+     +--------------+     +--------------+
|  Title   |---->|  World Map   |---->| Level Select |
|  Screen  |     |  (6 worlds)  |     |  (per world) |
+----------+     +--------------+     +--------------+
                                              |
                                              v
+----------+     +--------------+     +--------------+
|  Result  |<----|   IN-GAME    |<----| Pre-Level    |
|  Screen  |     |   (TD HUD)  |     |  (briefing)  |
+----------+     +--------------+     +--------------+
                        |
                        v
                 +--------------+
                 |  Pause Menu  |
                 +--------------+
```

---

## 4. Title Screen

Minimal, atmospheric. Full-screen pixel art scene that slowly animates (parallax clouds, flickering torches, etc.). Changes based on last played world or cycles through all six.

```
+--------------------------------------------------------+
|                                                          |
|                                                          |
|              AMIGO                                        |
|                     T D                                  |
|                                                          |
|                                                          |
|               > Continue                                 |
|                 New Game                                  |
|                 Settings                                  |
|                                                          |
|                                                          |
|                                          v0.1.0          |
+--------------------------------------------------------+
```

Gamepad: D-pad selects, A/Enter confirms. No complex navigation.

---

## 5. World Map

Stylized pixel art overview showing all 6 worlds as islands/regions on a fantasy map. Locked worlds are greyed out with a lock icon. Current world pulses subtly.

```
+--------------------------------------------------------+
|                                                          |
|     Caribbean               Lord of the Rings            |
|     [stars]                 [stars]                       |
|                                                          |
|            Dune                      Matrix               |
|           [locked]                  [locked]              |
|                                                          |
|        Game of Thrones           Stranger Things          |
|           [locked]                  [locked]              |
|                                                          |
|  [<- Back]                                               |
+--------------------------------------------------------+
```

Stars show completion progress (out of 5 levels per world). Worlds unlock sequentially or by completing N levels total.

---

## 6. Level Select

Per-world screen. Shows the world's pixel art landscape as background with level nodes connected by a path (like Kingdom Rush's level select). Each node shows star rating (0-3 stars).

```
+--------------------------------------------------------+
|  < Caribbean                                             |
|                                                          |
|     1---2---3---4---5                                    |
|     ***  **   *    .    .                                |
|                                                          |
|  +----------------------------+                          |
|  |  Level 3: Skull Cove       |                          |
|  |  * stars  Waves: 8         |                          |
|  |  Difficulty: ##...         |                          |
|  |                            |                          |
|  |  New towers: Kraken        |                          |
|  |  New enemies: Ghost Ship   |                          |
|  |                            |                          |
|  |  [> PLAY]    [Retry ***]   |                          |
|  +----------------------------+                          |
+--------------------------------------------------------+
```

Selecting a level shows an info card with wave count, difficulty, new unlocks, and best star rating. "Retry ***" option for perfectionists.

---

## 7. Pre-Level Briefing

Quick screen before gameplay starts. Shows the map preview, wave count, available towers, and any special mechanics for this level.

```
+--------------------------------------------------------+
|  Skull Cove                              Caribbean #3   |
|                                                          |
|  +--------------+  Waves: 8                              |
|  |              |  Gold: 300                              |
|  |  [Map        |  Lives: 20                              |
|  |   Preview]   |                                        |
|  |              |  Special:                               |
|  +--------------+  Water tiles slow enemies               |
|                                                          |
|  Towers:  Cannon Archer Mage Kraken                      |
|                                                          |
|  * Survive all waves                                     |
|  ** Lose fewer than 5 lives                              |
|  *** Lose no lives                                       |
|                                                          |
|              [> START]        [< Back]                    |
+--------------------------------------------------------+
```

---

## 8. In-Game HUD

The core gameplay UI. Minimal -- top bar + contextual radial menus.

### 8.1 Top Bar (always visible, 480x16 px)

```
+--------------------------------------------------------+
| Lives:20 | Wave 3/10 [progress bar] | Gold 450 | Pause Speed Settings |
+--------------------------------------------------------+
```

- **Lives**: Heart icon + number. Pulses red when hit, flashes when <=3.
- **Wave**: "Wave N/M" + progress bar showing enemies remaining in current wave.
- **Gold**: Coin icon + amount. Flashes green on income, flashes red when insufficient for action.
- **Speed**: Toggle 1x / 2x / 3x game speed. Current speed highlighted.
- **Pause**: Opens pause menu.

Between waves, a "NEXT WAVE" button replaces the progress bar area:

```
| Lives:20 | > NEXT WAVE (5s)     | Gold 450 | Pause Speed Settings |
```

Countdown timer shows auto-start. Click to start immediately (early start = gold bonus).

### 8.2 Wave Banner (transient)

On wave start, a banner slides in from the top for ~2 seconds:

```
         +---------------------+
         |   WAVE 3            |
         |   Skeleton Pirates   |
         +---------------------+
```

Boss waves get a larger, dramatic banner:

```
    +-----------------------------+
    |     BOSS WAVE               |
    |   Captain Blackbeard        |
    |   "Prepare to be boarded!"  |
    +-----------------------------+
```

### 8.3 Empty Tile Interaction (Radial Build Menu)

Click/tap an empty buildable tile -> radial menu appears centered on the tile:

```
              Archer
             (50g)

    Cannon                Mage
   (75g)                 (100g)

              Kraken
             (120g)
```

- 4-direction radial: Up/Right/Down/Left (maps to D-pad perfectly)
- Each slot shows: tower icon, name, cost
- Greyed out if not enough gold
- Hover/focus shows tower range preview on the map (transparent circle)
- Click icon or press D-pad direction -> tower placed instantly
- Click outside or press B/Escape -> cancel

**Tower range preview**: While the radial menu is open and a tower is focused, a semi-transparent circle shows the range on the map. This is the single most important visual feedback for placement decisions.

### 8.4 Tower Interaction (Radial Action Menu)

Click/tap an existing tower -> radial action menu:

```
              Upgrade
            (150g)

    Target               Sell
   Priority             (37g)
   [First]

              Info
```

- **Upgrade** (top): Shows cost. If tower has branching upgrades, a sub-menu appears.
- **Target** (left): Cycles through targeting modes (First -> Last -> Nearest -> Strongest -> Weakest -> Fastest). Shows current mode.
- **Sell** (right): Shows refund amount (50% of total invested). Confirm on click.
- **Info** (bottom): Opens tower info panel.

### 8.5 Tower Info Panel

Opens on info action or long-press on tower. Compact panel near the tower:

```
+-----------------------------+
|  Cannon Lv.2                |
|                              |
|  DMG: 45    RNG: 4.5        |
|  SPD: 1.2s  TGT: Strongest  |
|                              |
|  Kills: 23                   |
|  Total Damage: 1,840         |
|                              |
|  Upgrade Path:               |
|  [done] Iron Balls -> [done] |
|  Grape -> [ ] Explosive (300g)|
|                              |
|  [Close]                     |
+-----------------------------+
```

Shows stats, kill counter, damage dealt, and upgrade path progress.

### 8.6 Enemy HP Bars

Small HP bar above each enemy. Only visible when damaged:

```
    [####..]  <- green/yellow/red based on %
    [enemy]   <- enemy sprite
```

Boss enemies get a larger bar pinned to the top of the screen:

```
+--------------------------------------------------------+
| Lives:20 | Cpt. Blackbeard [############....] | Gold 450 |...|
+--------------------------------------------------------+
```

### 8.7 Damage Numbers (floating)

On hit, small floating numbers rise from the impact point:

```
    -45        <- white for normal
    -120       <- yellow for critical
    POISON     <- green for status effects
    IMMUNE     <- grey for immune
```

Pixel font, fades out over ~0.5s while floating upward.

### 8.8 Gold Income Feedback

When enemies die, floating "+10" gold numbers near the kill point. When selling, "+37" near the tower.

### 8.9 Build Zone Highlighting

When the build radial is open, all valid build tiles glow subtly (pulsing green outline). Invalid tiles (paths, water, occupied) stay normal.

---

## 9. Pause Menu

Overlay with slight darken/blur on the game behind:

```
+--------------------------------------------------------+
|                                                          |
|                                                          |
|      +------------------------------+                    |
|      |          PAUSED              |                    |
|      |                              |                    |
|      |     > Resume                 |                    |
|      |       Restart Level          |                    |
|      |       Settings               |                    |
|      |       Quit to Map            |                    |
|      |                              |                    |
|      |  Wave 3/10  Gold: 450        |                    |
|      |  Time: 4:23  Kills: 47       |                    |
|      +------------------------------+                    |
|                                                          |
+--------------------------------------------------------+
```

---

## 10. Result Screen

### Victory

```
+--------------------------------------------------------+
|                                                          |
|               VICTORY                                    |
|                                                          |
|              * * *                                       |
|          (all 3 stars earned!)                            |
|                                                          |
|    Lives: 18/20     Gold: 1,240                          |
|    Kills: 87        Time: 6:42                           |
|    Best Tower: Cannon Lv.3 (412 kills)                   |
|                                                          |
|    +------------------------------+                      |
|    |  NEW UNLOCK: Kraken Tower!   |                      |
|    |  Available in next level     |                      |
|    +------------------------------+                      |
|                                                          |
|         [> Next Level]   [Retry]   [Map]                 |
|                                                          |
+--------------------------------------------------------+
```

### Defeat

```
+--------------------------------------------------------+
|                                                          |
|              DEFEAT                                      |
|                                                          |
|        You survived to Wave 6/10                         |
|                                                          |
|    Lives: 0/20      Gold: 890                            |
|    Kills: 54        Time: 4:11                           |
|    Best Tower: Archer Lv.2 (203 kills)                   |
|                                                          |
|    TIP: Try placing towers near                          |
|    the path's curve for maximum                          |
|    coverage.                                             |
|                                                          |
|         [> Retry]    [Map]                               |
|                                                          |
+--------------------------------------------------------+
```

Defeat screen shows a contextual tip based on what went wrong (e.g., "enemies leaked at the second curve" -> suggest tower placement there).

---

## 11. Settings Screen

Accessible from title screen and pause menu:

```
+--------------------------------------------------------+
|  Settings                                                |
|                                                          |
|  Audio                                                   |
|    Master     [########..]  80%                          |
|    Music      [######....]  60%                          |
|    SFX        [########..]  80%                          |
|    Ambient    [######....]  60%                          |
|                                                          |
|  Display                                                 |
|    Fullscreen        [ON]                                |
|    Resolution        [1920x1080]                         |
|    Pixel Scaling     [Integer]                           |
|    Show FPS          [OFF]                               |
|                                                          |
|  Gameplay                                                |
|    Auto-Start Waves  [OFF]                               |
|    Damage Numbers    [ON]                                |
|    Show HP Bars      [Always / Damaged / Never]          |
|    Confirm Sell      [ON]                                |
|                                                          |
|  Controls                                                |
|    [View Keybinds]   [Rebind]                            |
|                                                          |
|  [< Back]                                                |
+--------------------------------------------------------+
```

---

## 12. World-Themed UI Frames

Every panel, button, and frame has world-specific art. Same layout, different skin:

| World | Frame Style | Primary Color | Accent | Font Feel |
|-------|------------|---------------|--------|-----------|
| Caribbean | Weathered wood planks, rope borders | Warm brown | Gold | Rugged serif |
| Lord of the Rings | Carved stone, elvish vine borders | Dark grey | Emerald | Elegant |
| Dune | Sandstone, geometric patterns | Sandy beige | Spice orange | Angular |
| Matrix | Black glass, neon green scan lines | Black | Neon green | Monospace |
| Game of Thrones | Iron/steel, riveted borders | Dark iron | Ice blue / fire red | Medieval |
| Stranger Things | 80s wood panel, Christmas lights | Brown/cream | Red flickering | Retro rounded |

The UI frame sprites are swapped when entering a world. Same layout, different atlas. This means every UI element needs to be designed once as a template, then skinned 6 times.

### Frame 9-Slice System

All panels use 9-slice sprites (corners + edges + fill):

```
+-------------+
| TL | T | TR |    TL, TR, BL, BR = fixed corner sprites
+----+---+----+    T, B, L, R = repeating edge sprites
| L  | F | R  |    F = repeating fill sprite
+----+---+----+
| BL | B | BR |    Total: 9 sprites per frame style
+----+---+----+    x 6 worlds = 54 frame sprites
```

---

## 13. Tower Upgrade UI

When a tower has branching upgrades, the upgrade radial expands into a mini upgrade tree:

### Linear Upgrades (Phase 1, simpler)

```
    Tower Lv.1 -> Lv.2 (100g) -> Lv.3 (200g) -> MAX

    Shown in radial as:
              Up
         Next Upgrade
           (100g)
           Lv.2
```

### Branching Upgrades (Phase 2+)

Two paths, can only pick one after a branch point:

```
              Path A
          Explosive Shots
             (300g)

    <- Back              Path B ->
                      Rapid Fire
                        (250g)
```

Selecting a path locks the other. The info panel shows both paths with the locked one greyed out.

---

## 14. Wave Preview

Before each wave (during the between-wave pause), a small preview appears in the top bar area:

```
| Lives:20 | NEXT: Orc x8 Skeleton x4 Boss x2  [> START (5s)]  | Gold 450 |
```

Icons show enemy types and counts for the upcoming wave. Players can plan tower placement based on what's coming.

For boss waves, a special preview:

```
| Lives:20 | WARNING BOSS: Captain Blackbeard  [> START]  | Gold 450 |
```

---

## 15. Notifications & Toasts

Small, transient messages for game events:

```
    +------------------+
    | Tower upgraded!   |    <- slides in from right, fades after 2s
    +------------------+

    +------------------+
    | WARNING Life lost!|    <- red tint, shakes slightly
    +------------------+

    +------------------+
    | Early bonus!      |    <- gold tint
    | +50 gold          |
    +------------------+
```

Stack from top-right, max 3 visible at once, oldest fades first.

---

## 16. Gamepad Controls

Full gamepad support. Cursor is a highlighted tile selector that snaps to the grid.

| Input | Action |
|-------|--------|
| Left Stick / D-pad | Move tile cursor |
| A (confirm) | Select tile -> open radial / confirm action |
| B (back) | Cancel radial / close panel / back |
| X | Quick-sell selected tower |
| Y | Cycle target priority on selected tower |
| RB | Next wave / speed toggle |
| LB | Zoom (if applicable) |
| Start | Pause menu |
| Triggers | Scroll tower list (if more than 4 towers) |

The radial menu maps directly to D-pad: Up = top tower, Right = right tower, etc. No analog aiming needed.

---

## 17. Keyboard + Mouse Controls

| Input | Action |
|-------|--------|
| Left Click | Select tile / confirm action |
| Right Click | Cancel / deselect |
| Mouse Wheel | Zoom |
| 1-9 | Quick-select tower type |
| Space | Start next wave |
| S | Sell selected tower |
| T | Cycle target priority |
| F | Toggle speed (1x / 2x / 3x) |
| Escape | Pause |

Number keys for quick tower placement: press "1" then click a tile to instantly place the first tower without opening the radial menu. Power-user shortcut.

---

## 18. Accessibility

- **Color-blind mode**: Enemy types distinguished by shape + icon, not just color. Status effects use both color and a text label.
- **Font size**: Bitmap font rendered at 1x and 2x for key numbers (lives, gold). Option to toggle large HUD.
- **Screen reader hints**: All UI elements have text labels (for future TTS support).
- **Auto-pause**: Game pauses when window loses focus.
- **Speed control**: 0.5x speed option for players who need more time.

---

## 19. Animation & Juice

### UI Feedback

- **Button press**: 1px down-shift + slight darken on click
- **Tower placement**: Brief flash + small particle burst at tile
- **Tower sell**: Tower sprite shrinks to nothing over 0.3s + gold particles float to HUD
- **Upgrade**: Tower sprite flashes white, then swaps to upgraded version with brief glow
- **Life lost**: Screen edge flashes red, lives counter shakes, enemy that leaked briefly highlighted
- **Wave clear**: Brief "WAVE CLEAR" text, gold tally animation
- **Gold change**: Numbers in HUD count up/down smoothly (not instant jump)

### Screen Transitions

- **Title -> World Map**: Fade to black, 0.3s
- **World Map -> Level**: Camera zooms into the selected world, crossfade
- **Level Select -> Game**: Map swoops in from the level node position
- **Victory/Defeat**: Game dims, result panel slides up from bottom

---

## 20. Mockup: Full In-Game Frame

```
+------------------------------------------------------------+
| Lives:17 | Wave 5/10 [#####.....]  | Gold 320  | Pause Speed Settings |
+------------------------------------------------------------+
|                                                              |
|    ~~  ~~  Palm  ~~  ~~  ~~  Palm  ~~  ~~  ~~               |
|    ~~  ~~  ..  ..  ..  ~~  ~~  ~~  ~~  ~~                   |
|    ~~  ~~  ..  Archer  ..  ..  ~~  ~~  ~~  ~~               |
|    ~~  ~~  ..  ..  ..  ..  ..  ~~  ~~  ~~                   |
|    Water  ~~  ~~  ..  Cannon  ..  ..  ~~  Palm  ~~          |
|    Water  Water  ~~  ..  ..  ..  ..  ..  ~~  ~~             |
|    Water  Water  ~~  ~~  ..  ..  Mage  ..  ~~  ~~           |
|    Water  ~~  ~~  ~~  ~~  ..  ..  ..  ~~  ~~                |
|    ~~  ~~  ~~  ~~  ~~  ~~  ..  ..  ~~  ~~                   |
|    ~~  ~~  Palm  ~~  ~~  ~~  ~~  ..  ~~  ~~   Enemies ->   |
|    ~~  ~~  ~~  ~~  ~~  ~~  ~~  ..  ~~  ~~                   |
|                                                              |
|                  +-----+                                     |
|                  | -45  |  <- damage number floating         |
|                  +-----+                                     |
|                                         +10 gold             |
|                                                              |
|                        [Selected: Cannon Lv.2]               |
|                  +- Target -- Upgrade -- Sell -+             |
|                  |                             |  <- radial  |
|                  +-------- Info ---------------+             |
+------------------------------------------------------------+
```

---

## 21. UI Asset Requirements

### Per World (x6)

| Asset | Count | Size | Notes |
|-------|-------|------|-------|
| 9-slice frame | 9 sprites | 8x8 corners, 8x1 edges | Panel backgrounds |
| Button (normal, hover, pressed, disabled) | 4 states | 32x12 | Standard button |
| Tower icons | per tower type | 16x16 | Radial menu + HUD |
| Enemy icons | per enemy type | 12x12 | Wave preview |
| Star (empty, filled) | 2 | 8x8 | Level select |
| Heart icon | 1 | 8x8 | Lives |
| Coin icon | 1 | 8x8 | Gold |
| Wave banner | 1 | 160x24 | World-themed |
| Cursor/selector | 1 | 16x16 | Tile highlight |

### Shared (x1)

| Asset | Count | Size |
|-------|-------|------|
| Bitmap font (Press Start 2P or similar) | 1 atlas | variable |
| Speed icons (1x, 2x, 3x) | 3 | 12x12 |
| Pause icon | 1 | 12x12 |
| Settings gear | 1 | 12x12 |
| Radial menu ring | 1 | 64x64 |
| HP bar (background + fill) | 2 | 16x2 |
| Damage number font | 1 atlas | variable |
| Toast notification frame | 9-slice | 8x8 |

### Total Estimate

~120-150 unique UI sprites per world, ~50 shared sprites. With 6 worlds: ~800 total UI sprites. All 16-color palette per world, consistent with amigo_artgen style definitions.

---

*For the engine specification, see the engine specs. For game mechanics, see [games/td/design](../td/design.md). For art generation, see [artgen](../../ai-pipelines/artgen.md). For audio, see [audiogen](../../ai-pipelines/audiogen.md).*
