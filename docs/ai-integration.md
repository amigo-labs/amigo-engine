# AI Integration Guide

This guide covers how to integrate AI tooling -- specifically Claude Code via MCP -- with the Amigo Engine. The architecture exposes engine internals through a two-layer system and three MCP servers, enabling AI-assisted gameplay balancing, level design, art generation, and audio generation.

---

## Architecture

Amigo Engine uses a two-layer architecture for AI integration:

1. **amigo_api** -- A JSON-RPC 2.0 IPC interface that exposes engine internals (simulation state, editor commands, asset pipeline) to external processes.
2. **amigo_mcp** -- An MCP (Model Context Protocol) bridge that wraps `amigo_api` and presents it as tool calls consumable by Claude Code.

Three MCP servers divide responsibilities:

| Server             | Purpose                              | Backend Dependency              |
|--------------------|--------------------------------------|---------------------------------|
| `amigo`            | Engine control, simulation, editor   | Amigo Engine (local)            |
| `amigo-artgen`     | Sprite, tileset, and inpaint generation | ComfyUI (`localhost:8188`)   |
| `amigo-audiogen`   | Music and SFX generation             | ACE-Step/AudioGen (`localhost:7860`) |

---

## Claude Code MCP Configuration

Add the following to your Claude Code MCP config:

```json
{
  "mcpServers": {
    "amigo": {
      "command": "amigo",
      "args": ["mcp-server", "--port", "9999"]
    },
    "amigo-artgen": {
      "command": "amigo-artgen",
      "args": ["--server", "http://localhost:8188"]
    },
    "amigo-audiogen": {
      "command": "amigo-audiogen",
      "args": ["--acestep", "http://localhost:7860"]
    }
  }
}
```

Once configured, all tools listed below become available as MCP tool calls within Claude Code.

---

## MCP Tools Reference

### Observation

These tools read engine state without modifying it.

| Tool                    | Parameters                          | Description                                |
|-------------------------|-------------------------------------|--------------------------------------------|
| `amigo_screenshot`      | `path`, `overlays?`, `area?`        | Capture a screenshot, optionally with debug overlays or a specific area. |
| `amigo_get_state`       |                                     | Return the full simulation state as JSON.  |
| `amigo_list_entities`   | `filter?`, `near?`, `radius?`       | List entities, optionally filtered by type or spatial proximity. |
| `amigo_inspect_entity`  | `id`                                | Return detailed data for a single entity.  |
| `amigo_perf`            |                                     | Return performance metrics (FPS, frame times, memory). |

### Simulation

These tools mutate the running simulation.

| Tool                  | Parameters                    | Description                                  |
|-----------------------|-------------------------------|----------------------------------------------|
| `amigo_place_tower`   | `x`, `y`, `tower_type`       | Place a tower at the given coordinates.      |
| `amigo_sell_tower`    | `tower_id`                   | Sell an existing tower.                      |
| `amigo_upgrade_tower` | `tower_id`, `path`           | Upgrade a tower along a specified path.      |
| `amigo_start_wave`    |                               | Start the next enemy wave.                   |
| `amigo_tick`          | `count`                       | Headless fast-forward by N simulation ticks. |
| `amigo_set_speed`     | `multiplier`                  | Set the simulation speed multiplier.         |
| `amigo_pause`         |                               | Pause the simulation.                        |
| `amigo_unpause`       |                               | Unpause the simulation.                      |
| `amigo_spawn`         | `type`, `subtype`, `pos`      | Spawn an entity of the given type at a position. |

### Editor

These tools modify the level in the editor.

| Tool                         | Parameters                     | Description                              |
|------------------------------|--------------------------------|------------------------------------------|
| `amigo_editor_paint_tile`    | `layer`, `x`, `y`, `tile`     | Paint a single tile on a layer.          |
| `amigo_editor_fill_rect`     | *(rect coords, layer, tile)*  | Fill a rectangular region with a tile.   |
| `amigo_editor_place_entity`  | *(entity type, position, ...)*| Place an entity in the editor.           |
| `amigo_editor_add_path`      | `points`                       | Define an enemy path as a list of points.|
| `amigo_editor_save`          | `path`                         | Save the current level to disk.          |
| `amigo_editor_load`          | `path`                         | Load a level from disk.                  |

### Art Generation (amigo-artgen MCP)

Requires ComfyUI running at `localhost:8188` (or a remote endpoint passed via `--server`).

| Tool                          | Parameters                        | Description                            |
|-------------------------------|-----------------------------------|----------------------------------------|
| `amigo_artgen_generate_sprite`| `prompt`, `style`, `size`         | Generate a sprite from a text prompt.  |
| `amigo_artgen_generate_tileset`| *(prompt, style, grid dims, ...)* | Generate a full tileset.              |
| `amigo_artgen_inpaint`        | *(image, mask, prompt, ...)*      | Inpaint a region of an existing image.|

### Audio Generation (amigo-audiogen MCP)

Requires ACE-Step running at `localhost:7860` (or a remote endpoint passed via `--acestep`).

| Tool                            | Parameters                  | Description                              |
|---------------------------------|-----------------------------|------------------------------------------|
| `amigo_audiogen_generate_music` | `prompt`, `duration`, `bpm` | Generate a music track from a text prompt.|
| `amigo_audiogen_generate_sfx`   | `prompt`                    | Generate a sound effect from a text prompt.|

---

## Example Workflows

### 1. Building a Level from Scratch

This workflow uses editor and observation tools to create a playable level entirely through MCP calls.

```text
Step 1 -- Create the terrain
    amigo_editor_fill_rect(layer="ground", x1=0, y1=0, x2=31, y2=31, tile="grass")
    amigo_editor_fill_rect(layer="ground", x1=10, y1=0, x2=12, y2=31, tile="dirt_path")

Step 2 -- Define the enemy path
    amigo_editor_add_path(points=[[11, 0], [11, 10], [20, 10], [20, 25], [11, 25], [11, 31]])

Step 3 -- Place entities (spawn point, base, decorations)
    amigo_editor_place_entity(type="spawn_point", x=11, y=0)
    amigo_editor_place_entity(type="base", x=11, y=31)
    amigo_editor_place_entity(type="tree", x=5, y=5)

Step 4 -- Verify visually
    amigo_screenshot(path="level_preview.png", overlays=["grid", "paths"])

Step 5 -- Save
    amigo_editor_save(path="levels/ai_designed_01.level")
```

### 2. Balancing Gameplay with Headless Simulation

Use headless fast-forward to test tower placements and wave difficulty without rendering.

```text
Step 1 -- Load the level and inspect starting state
    amigo_editor_load(path="levels/ai_designed_01.level")
    amigo_get_state()

Step 2 -- Place a defensive setup
    amigo_place_tower(x=13, y=8, tower_type="archer")
    amigo_place_tower(x=9, y=10, tower_type="cannon")
    amigo_place_tower(x=21, y=12, tower_type="frost")

Step 3 -- Run a wave headlessly
    amigo_start_wave()
    amigo_tick(count=3000)

Step 4 -- Evaluate results
    amigo_get_state()   # check remaining HP, gold earned, enemies leaked
    amigo_perf()        # confirm tick performance is acceptable

Step 5 -- Iterate
    # If too easy: spawn additional enemies or remove a tower.
    # If too hard: add towers or downgrade enemy subtypes.
    amigo_sell_tower(tower_id=2)
    amigo_spawn(type="enemy", subtype="armored_orc", pos=[11, 0])
    amigo_start_wave()
    amigo_tick(count=3000)
    amigo_get_state()
```

This loop can run many iterations quickly since `amigo_tick` advances the simulation without rendering.

### 3. Generating Art Assets with ComfyUI

Use the `amigo-artgen` MCP server to produce game-ready sprites and tilesets.

```text
Step 1 -- Generate a tower sprite
    amigo_artgen_generate_sprite(
        prompt="medieval stone archer tower, top-down, pixel art",
        style="pixel_art_32",
        size=[64, 64]
    )

Step 2 -- Generate a ground tileset
    amigo_artgen_generate_tileset(
        prompt="grassy plains with dirt path transitions, top-down",
        style="pixel_art_32",
        grid=[4, 4],
        tile_size=[32, 32]
    )

Step 3 -- Fix an existing sprite via inpainting
    amigo_artgen_inpaint(
        image="sprites/cannon_tower.png",
        mask="sprites/cannon_tower_mask.png",
        prompt="add glowing blue runes to the cannon barrel, pixel art"
    )
```

Generated assets are saved into the project's asset directory and can be referenced by the editor tools immediately.

---

## Hardware Requirements

| Component                  | Requirement                                            |
|----------------------------|--------------------------------------------------------|
| **Amigo Engine**           | Any modern GPU with Vulkan, DX12, or Metal support.    |
| **ComfyUI (art gen)**      | NVIDIA GPU with 8 GB+ VRAM recommended.                |
| **ACE-Step (audio gen)**   | NVIDIA GPU with 8 GB+ VRAM recommended.                |

The engine itself runs on all major GPU vendors. The generative AI backends (ComfyUI and ACE-Step) perform best on NVIDIA hardware due to CUDA dependencies, though CPU-only fallback is possible at significantly reduced speed.
