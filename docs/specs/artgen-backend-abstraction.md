---
status: in-progress
crate: amigo_artgen
depends_on: ["tools/artgen"]
last_updated: 2026-03-21
type: adr
---

# AP-15: ArtGen Backend-Abstraktion

> ADR — 2026-03-21

## Kontext

Die aktuelle `amigo_artgen` ist hart an ComfyUI gekoppelt — Workflows werden
als ComfyUI-Node-Graphen gebaut, der User muss ComfyUI kennen und manuell
starten. Das Ziel: **ComfyUI bleibt als Inferenz-Orchestrator, aber komplett
unter der Haube.** Der User wählt nur ein Modell in `amigo.toml` und die
Engine managed den Rest.

Zusätzlich: Bisher nur Pixel Art als Output. Jetzt auch **Raster Art** als
Output-Modus (ohne Pixel-Art-Postprocessing wie Palette-Clamp, Anti-Aliasing-
Entfernung etc.).

## Entscheidung

### Drei Backends

1. **Qwen-Image** (Default) — 7B Parameter, beste Qualität/Size Ratio, Apache 2.0
2. **FLUX.2 Klein** (4B) — Kompakt, schnell, großes LoRA-Ökosystem
3. **Custom** — User gibt eine Workflow-URL an (ComfyUI API-kompatibel oder eigener Endpunkt)

### Architektur

```
amigo.toml [art]
    │
    ▼
ImageBackend enum (qwen-image | flux2-klein | custom)
    │
    ▼
WorkflowBuilder (generiert modell-spezifische ComfyUI-Workflows)
    │
    ▼
ComfyUiClient (HTTP → localhost ComfyUI, auto-managed)
    │
    ▼
PostProcessor (pixel-art ODER raster-art je nach art_mode)
```

ComfyUI wird als Child-Process gestartet und gemanaged. Der User interagiert
nie direkt mit ComfyUI.

### Zwei Art-Modi

- **Pixel**: Vollständige Pipeline (RemoveAA, PaletteClamp, Outline, etc.)
- **Raster**: Nur ForceDimensions + CleanupTransparency

## Scope

### In Scope

- `ImageBackend` enum mit Qwen-Image, FLUX.2 Klein, Custom
- `ArtMode` enum mit Pixel, Raster
- Backend-spezifische ComfyUI-Workflow-Builder (verschiedene Node-Graphen pro Modell)
- `ComfyUiLifecycle`: Auto-Start/Health-Check/Auto-Stop von ComfyUI
- Config-Erweiterung: `backend`, `art_mode`, `custom_endpoint`, `custom_workflow_url`
- Postprocessing-Weiche basierend auf `ArtMode`
- Neues MCP-Tool: `amigo_artgen_list_backends`
- Setup-Erweiterung: Modell-Downloads für Qwen-Image und FLUX.2 Klein

### Out of Scope

- Eigene Inferenz-Runtime (wir nutzen ComfyUI)
- Trainieren/Fine-Tuning von Modellen
- Web-basierte UI für Modell-Auswahl
- Weitere Modelle (SDXL, SD3 etc.) — können später als weitere Backend-Varianten ergänzt werden

## Config-Schema

```toml
[art]
backend = "qwen-image"       # "qwen-image" | "flux2-klein" | "custom"
art_mode = "pixel"            # "pixel" | "raster"
default_sprite_size = 32
default_style = "caribbean"
default_palette = "standard"

# Nur bei backend = "custom":
custom_endpoint = "http://localhost:8188"
custom_workflow_url = "https://example.com/my-workflow.json"
```

## Workflow-Builder pro Backend

**Qwen-Image:** Checkpoint: `qwen-image-7b-Q4_K_M.gguf`. Braucht `UNETLoader`
(Diffusion-Model), `DualCLIPLoader` (Qwen2.5-VL CLIP), `VAELoader`. Sampler:
`KSampler` mit `euler` scheduler, 28 steps, CFG 7.0.

**FLUX.2 Klein:** Flow-Matching-basiert. Checkpoint: `flux2-klein-4b-fp8.safetensors`.
Braucht `UNETLoader`, `DualCLIPLoader` (T5 + CLIP-L), `VAELoader`.
`FluxGuidance` Node (guidance_scale 3.5). `BasicScheduler` mit `sgm_uniform`,
28 steps.

**Custom:** Lädt Workflow-JSON von URL, ersetzt `{{PROMPT}}`, `{{NEGATIVE}}`,
`{{WIDTH}}`, `{{HEIGHT}}`, `{{SEED}}` Platzhalter, sendet an ComfyUI-Endpunkt.

## Akzeptanzkriterien

- [ ] AC1: `ImageBackend` enum mit drei Varianten, `Default` = QwenImage
- [ ] AC2: `ArtMode` enum mit Pixel/Raster, `Default` = Pixel
- [ ] AC3: `ArtRequest` hat `backend` und `art_mode` Felder
- [ ] AC4: `build_workflow()` dispatcht basierend auf Backend zu korrektem Builder
- [ ] AC5: Qwen-Image Workflow enthält `UNETLoader` + `DualCLIPLoader` Nodes
- [ ] AC6: FLUX.2 Klein Workflow enthält `FluxGuidance` Node + `sgm_uniform` Scheduler
- [ ] AC7: Custom Workflow lädt JSON und ersetzt Platzhalter
- [ ] AC8: `ComfyUiLifecycle::ensure_running()` startet ComfyUI wenn Port nicht erreichbar
- [ ] AC9: `ComfyUiLifecycle::shutdown()` terminiert Child-Process sauber
- [ ] AC10: Postprocessing-Weiche: Pixel-Modus → volle Pipeline, Raster-Modus → nur Dimensionen/Transparenz
- [ ] AC11: `amigo_artgen_list_backends` MCP-Tool gibt alle drei Backends zurück
- [ ] AC12: Config liest `backend` und `art_mode` aus `amigo.toml [art]`
- [ ] AC13: Bestehende Tests bleiben grün (Rückwärtskompatibilität)
- [ ] AC14: Neue Tests für jeden Workflow-Builder
- [ ] AC15: `REQUIREMENTS_ARTGEN` enthält Abhängigkeiten für Modell-Downloads
- [ ] AC16: `cargo test -p amigo_artgen` — alle Tests grün
- [ ] AC17: `cargo build -p amigo_artgen` — kompiliert ohne Warnungen

## Technische Notizen

### Qwen-Image Node-Graph (ComfyUI)

```
UNETLoader → KSampler ← DualCLIPLoader (positive/negative)
                ↑
          EmptyLatentImage
                ↓
          VAEDecode → SaveImage
```

### FLUX.2 Klein Node-Graph (ComfyUI)

```
UNETLoader → FluxGuidance → BasicGuider → SamplerCustomAdvanced
                                               ↑
DualCLIPLoader → CLIPTextEncode            BasicScheduler (sgm_uniform)
                                               ↑
                                         EmptySD3LatentImage
                                               ↓
                                         VAEDecode → SaveImage
```

### Dateien

| Datei | Änderung |
|-------|----------|
| `lib.rs` | `ImageBackend`, `ArtMode` enums + `ArtRequest` Felder |
| `config.rs` | `backend`, `art_mode`, `custom_endpoint`, `custom_workflow_url` |
| `workflows.rs` | Qwen/FLUX/Custom Workflow-Builder |
| `comfyui.rs` | `ComfyUiLifecycle` Auto-Start/Stop |
| `tools.rs` | Backend-aware Dispatch, `list_backends` Tool |
| `main.rs` | Backend-Init aus Config |
| `postprocess.rs` | `ArtMode`-Weiche |
| `setup.rs` (CLI) | Requirements + Modell-Downloads |
| `amigo.toml` | `[art]` Sektion |
