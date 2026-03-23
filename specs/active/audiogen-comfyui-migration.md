---
number: "0014"
title: "AudioGen auf ComfyUI migrieren — Qwen3-TTS als einziges TTS-Modell"
status: implementing
date: 2026-03-22
type: adr
crate: amigo_audiogen
depends_on: ["ai-pipelines/audiogen", "tools/amigo_artgen"]
last_updated: 2026-03-22
---

# ADR-0014: AudioGen auf ComfyUI migrieren — Qwen3-TTS als einziges TTS-Modell

## Status

implementing

## Context

`amigo_audiogen` (`tools/amigo_audiogen/src/`) generiert aktuell Musik und SFX
über **direkte Gradio-HTTP-Calls** zu lokalen ACE-Step- und AudioGen-Servern.
Das funktioniert, hat aber Nachteile:

- **Zwei separate Server** müssen laufen (ACE-Step Gradio + AudioGen Gradio),
  zusätzlich zum ComfyUI-Server der bereits für `amigo_artgen` läuft.
- **Kein TTS** — Sprachausgabe (NPC-Dialog, Narrator, UI-Feedback) fehlt komplett.
  Die `SfxCategory::Voice` in `tools/amigo_audiogen/src/lib.rs` erzeugt nur
  generische Sound-Effekte, keine gesprochene Sprache.
- **Keine einheitliche Pipeline** — Artgen nutzt ComfyUI, Audiogen nutzt Gradio.
  Zwei verschiedene Lifecycle-Patterns, zwei Konfigurationsmodelle.

Gleichzeitig existiert in `tools/amigo_artgen/src/comfyui.rs` bereits eine
ausgereifte ComfyUI-Integration (`ComfyUiClient`, `ComfyUiLifecycle`) die
Prompt-Queueing, Polling und Output-Retrieval abdeckt.

Alle drei Backends (ACE-Step, AudioGen/Stable Audio, Qwen3-TTS) haben
ComfyUI-Custom-Nodes:

| Backend | ComfyUI-Node | Zweck |
|---------|-------------|-------|
| ACE-Step | `ComfyUI-ACEStepWrapper` | Musik-Generierung |
| Stable Audio Open | `ComfyUI-StableAudioSampler` | SFX-Generierung |
| Qwen3-TTS 1.7B | `ComfyUI-Qwen-TTS` | Sprache (DE+EN, Voice Cloning, Emotion via Instructions) |

## Decision

**Alle Audio-Generierung über ComfyUI laufen lassen und Qwen3-TTS als einziges
TTS-Modell integrieren.**

### Warum nur Qwen3-TTS (kein Dia, kein weiteres Modell)?

- **10 Sprachen nativ** (DE + EN + 8 weitere) — deckt unsere Anforderungen ab.
- **Delivery-Instructions** steuern Emotion/Stil: `"speak slowly with sadness"`,
  `"whisper with urgency"` — kein Tag-System nötig.
- **Voice Cloning** über 10s Referenz-Audio — verschiedene NPC-Stimmen ohne
  Modellwechsel.
- **1.7B Parameter** — läuft auf Consumer-GPU neben den anderen Modellen.
- **YAGNI** — Dia (nur Englisch) oder weitere Modelle erst wenn Qwen3-TTS
  nachweislich nicht ausreicht.

### Architektur nach Migration

```
amigo_audiogen
├── comfyui.rs          ← Shared ComfyUI-Client (aus artgen extrahiert)
├── workflows/
│   ├── music.rs        ← ACE-Step ComfyUI-Workflow
│   ├── sfx.rs          ← Stable Audio ComfyUI-Workflow
│   └── tts.rs          ← Qwen3-TTS ComfyUI-Workflow
├── lib.rs              ← MusicRequest, SfxRequest, TtsRequest, AudioBackend
├── processing.rs       ← Bestehende Post-Processing Pipeline (unverändert)
├── stems.rs            ← Demucs Stem-Separation (unverändert)
├── clean_mode.rs       ← Clean-Mode Pipeline (unverändert)
└── tools.rs            ← MCP-Tools (erweitert um TTS-Tools)
```

### Neue/Geänderte Typen

```rust
/// Audio-Backend-Auswahl (ersetzt die impliziten Gradio-Clients).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum AudioBackend {
    /// ACE-Step via ComfyUI für Musik.
    AceStep,
    /// Stable Audio Open via ComfyUI für SFX.
    StableAudio,
    /// Qwen3-TTS 1.7B via ComfyUI für Sprache.
    Qwen3Tts,
}

/// TTS-Anfrage.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsRequest {
    /// Der zu sprechende Text.
    pub text: String,
    /// Sprache (BCP-47). Default: "de-DE".
    pub language: String,
    /// Delivery-Instruction für Emotion/Stil.
    /// z.B. "speak with anger", "whisper softly", "excited and fast".
    pub delivery: Option<String>,
    /// Pfad zu Referenz-Audio für Voice Cloning (10s reichen).
    /// Wenn None → Default-Stimme des Modells.
    pub reference_audio: Option<String>,
    /// Sprechername für konsistente Zuordnung (z.B. "narrator", "npc_guard").
    pub speaker_id: Option<String>,
    /// Ausgabeformat.
    pub format: AudioFormat,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub enum AudioFormat {
    #[default]
    Wav,
    Ogg,
}

/// TTS-Ergebnis.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TtsResult {
    /// Pfad zur generierten Audio-Datei.
    pub output_path: String,
    /// Dauer in Sekunden.
    pub duration_secs: f32,
    /// Generierungszeit in ms.
    pub generation_time_ms: u64,
}

// ---------------------------------------------------------------------------
// Voice Creation & Management
// ---------------------------------------------------------------------------

/// Ein gespeichertes Stimmprofil.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VoiceProfile {
    /// Eindeutiger Name (z.B. "old_wizard", "young_princess", "narrator_de").
    pub name: String,
    /// Pfad zum Referenz-Audio (WAV, 10-30s empfohlen).
    pub reference_audio: String,
    /// Standard-Sprache für diese Stimme.
    pub default_language: String,
    /// Standard-Delivery-Instruction (z.B. "speak slowly with gravitas").
    pub default_delivery: Option<String>,
    /// Beschreibung der Stimme (für MCP-Tool-Anzeige).
    pub description: Option<String>,
}

/// Anfrage zum Erstellen eines Voice-Profils.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateVoiceRequest {
    /// Name der neuen Stimme.
    pub name: String,
    /// Referenz-Audio: entweder Pfad zu bestehender Datei oder "record"
    /// für Aufnahme über Mikrofon.
    pub reference_audio: String,
    /// Sprache. Default: "de-DE".
    pub language: String,
    /// Default-Delivery für diese Stimme.
    pub default_delivery: Option<String>,
    /// Beschreibung.
    pub description: Option<String>,
    /// Optionaler Test-Text — wird nach Erstellung gesprochen um die
    /// Stimme zu validieren.
    pub test_text: Option<String>,
}

/// Ergebnis der Voice-Erstellung.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreateVoiceResult {
    /// Das gespeicherte Profil.
    pub profile: VoiceProfile,
    /// Pfad zur Test-Audio-Datei (wenn test_text gesetzt war).
    pub test_audio: Option<String>,
}
```

### Voice-Speicherung

Voice-Profile werden als RON-Dateien im Projekt gespeichert:

```
assets/voices/
├── voices.ron            ← Registry aller Profile
├── old_wizard.wav        ← Referenz-Audio
├── young_princess.wav
└── narrator_de.wav
```

`voices.ron`:
```ron
VoiceRegistry(
    voices: {
        "old_wizard": VoiceProfile(
            name: "old_wizard",
            reference_audio: "assets/voices/old_wizard.wav",
            default_language: "de-DE",
            default_delivery: Some("speak slowly with a deep, gravelly voice"),
            description: Some("Alter Zauberer, tiefe raue Stimme"),
        ),
        "narrator_de": VoiceProfile(
            name: "narrator_de",
            reference_audio: "assets/voices/narrator_de.wav",
            default_language: "de-DE",
            default_delivery: Some("speak clearly and calmly"),
            description: Some("Neutraler deutscher Erzähler"),
        ),
    }
)
```

Beim `TtsRequest` reicht dann `speaker_id: Some("old_wizard")` — das System
löst automatisch Referenz-Audio und Default-Delivery auf.

### Shared ComfyUI-Client

`ComfyUiClient` und `ComfyUiConfig` aus `tools/amigo_artgen/src/comfyui.rs`
werden in ein shared Modul extrahiert (entweder eigener Mini-Crate
`amigo_comfyui` oder als re-export). Beide Tools (artgen + audiogen) nutzen
denselben Client und dieselbe ComfyUI-Instanz.

Der Output-Typ wird generalisiert:

```rust
/// ComfyUI-Output — entweder Bild oder Audio.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ComfyOutput {
    Image(OutputImage),
    Audio(OutputAudio),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputAudio {
    pub filename: String,
    pub subfolder: String,
    pub format: String,  // "wav", "ogg", "mp3"
}
```

### Neue MCP-Tools

| Tool | Beschreibung |
|------|-------------|
| `amigo_audiogen_generate_tts` | Text → Sprache generieren |
| `amigo_audiogen_create_voice` | Neues Stimmprofil erstellen (Referenz-Audio + Metadaten) |
| `amigo_audiogen_list_voices` | Alle gespeicherten Stimmprofile auflisten |
| `amigo_audiogen_preview_voice` | Kurze Vorschau einer Stimme mit Test-Text |
| `amigo_audiogen_delete_voice` | Stimmprofil entfernen |

### Alternatives Considered

1. **Voicebox als Abstraktions-Layer über mehrere TTS-Modelle** — Abgelehnt.
   Zusätzliche Abstraktionsschicht ohne Mehrwert wenn nur ein Modell genutzt
   wird. Unnötige Komplexität.

2. **Dia v1.6B als zweites TTS-Modell für emotionale Dialoge** — Verschoben.
   Dia ist nur Englisch. Deutsche Texte müssten übersetzt werden, aber die
   Audio-Ausgabe wäre dann Englisch. Qwen3-TTS kann Emotionen über
   Delivery-Instructions steuern. Dia wird erst evaluiert wenn Qwen3-TTS
   bei Multi-Speaker-Dialogen nicht überzeugt.

3. **Gradio-Server beibehalten, nur TTS hinzufügen** — Abgelehnt.
   Drei separate Server (ACE-Step + AudioGen + Qwen3-TTS) statt einem
   einzigen ComfyUI. Mehr Ressourcen, mehr Konfiguration, keine Pipeline-
   Komposition.

## Migration Path

1. **Shared ComfyUI-Client extrahieren** — `ComfyUiClient`, `ComfyUiConfig`,
   `ComfyPrompt`, `QueueResponse`, `PromptStatus` aus
   `tools/amigo_artgen/src/comfyui.rs` in neues Modul `amigo_comfyui`
   verschieben. Artgen importiert von dort.
   Verify: `cargo check --workspace` kompiliert. Artgen-Tests laufen weiter.

2. **TTS-Workflow-Builder implementieren** — `workflows/tts.rs` erstellen:
   Baut ComfyUI-Prompt-Graph für Qwen3-TTS Node (`QwenTTSNode` →
   `SaveAudio`). Nimmt `TtsRequest` entgegen.
   Verify: Unit-Test der den Workflow-JSON validiert (alle erwarteten Nodes
   vorhanden, Verbindungen korrekt).

3. **`TtsRequest`/`TtsResult` + `generate_tts()`** — In `lib.rs` die neuen
   Typen hinzufügen. `generate_tts()` baut Workflow, queued über
   `ComfyUiClient`, polled, gibt `TtsResult` zurück.
   Verify: Integration-Test mit laufendem ComfyUI + Qwen-TTS Node.

4. (rough) **Musik-Workflow auf ComfyUI umstellen** — `acestep.rs` durch
   `workflows/music.rs` ersetzen. `MusicRequest` bleibt gleich, Backend
   wechselt von Gradio auf ComfyUI.

5. (rough) **SFX-Workflow auf ComfyUI umstellen** — `audiogen.rs` durch
   `workflows/sfx.rs` ersetzen. Stable Audio Open statt AudioGen (bessere
   ComfyUI-Integration).

6. (rough) **MCP-Tools erweitern** — `generate_tts`, `list_voices`,
   `preview_voice` in `tools.rs` registrieren.

7. (rough) **Alte Gradio-Clients entfernen** — `acestep.rs` und `audiogen.rs`
   löschen wenn alle Workflows über ComfyUI laufen.

## Abort Criteria

- Wenn Qwen3-TTS **kein Deutsch** produzieren kann das verständlich klingt → anderes TTS-Modell evaluieren.
- Wenn ComfyUI-Audio-Nodes **>30s Latenz** für einen 5s TTS-Clip haben → Gradio beibehalten, nur TTS über ComfyUI.
- Wenn der Qwen3-TTS ComfyUI-Node **nicht mit aktuellem ComfyUI** (≥ März 2026) kompatibel ist → eigenen Node wrappen oder direkte Python-Bridge.

## Consequences

### Positive
- **Ein Server** für alle AI-Generation (Bilder + Musik + SFX + Sprache).
- **TTS endlich verfügbar** — NPC-Dialog, Narrator, Tutorial-Stimme.
- **Pipeline-Komposition** — in ComfyUI können Audio-Nodes verkettet werden
  (z.B. TTS → Reverb → Normalize in einem Workflow).
- **Konsistentes Pattern** — gleiche Client/Lifecycle/Workflow-Architektur
  wie Artgen.
- **Voice Cloning** — verschiedene NPC-Stimmen über Referenz-Audio, kein
  Modellwechsel nötig.

### Negative / Trade-offs
- **ComfyUI Custom Nodes nötig** — drei zusätzliche Node-Pakete installieren.
  Aber: das gilt auch für Artgen (Qwen-Image Node).
- **ComfyUI wird Single-Point-of-Failure** — wenn ComfyUI crasht, keine
  AI-Generation mehr. Mitigation: `ComfyUiLifecycle` hat bereits auto-restart.
- **Nur ein TTS-Modell** — wenn Qwen3-TTS bei bestimmten Szenarien schwächelt,
  muss nachträglich ein zweites Modell integriert werden. Akzeptables Risiko
  durch YAGNI-Prinzip.

## Acceptance Criteria

- [ ] `ComfyUiClient` liegt in eigenem Modul, wird von artgen und audiogen importiert
- [ ] `TtsRequest` → `TtsResult` erzeugt WAV-Datei über ComfyUI + Qwen3-TTS
- [ ] Deutsche Sprachausgabe klingt verständlich (manueller Check)
- [ ] Delivery-Instruction ändert hörbar den Stil (z.B. "whisper" vs "shout")
- [ ] Voice Cloning mit 10s Referenz produziert konsistente Stimme
- [ ] `CreateVoiceRequest` speichert Profil + Referenz-Audio unter `assets/voices/`
- [ ] `voices.ron` Registry wird korrekt gelesen und geschrieben
- [ ] `speaker_id` in `TtsRequest` löst Profil automatisch auf (Referenz + Delivery)
- [ ] MCP-Tool `create_voice` erstellt Profil und optional Test-Audio
- [ ] Musik-Generierung über ComfyUI statt Gradio funktioniert
- [ ] SFX-Generierung über ComfyUI statt Gradio funktioniert
- [ ] `cargo check --workspace` kompiliert
- [ ] `cargo test --workspace` grün
- [ ] `cargo clippy --workspace -- -D warnings` sauber
- [ ] `cargo fmt --all --check` sauber
- [ ] Alte Gradio-Clients (`acestep.rs`, `audiogen.rs`) entfernt

## Updates

- 2026-03-22: Extracted ComfyUiClient into shared `amigo_comfyui` crate. Added OutputAudio + ComfyOutput types. Artgen re-exports from shared crate.
- 2026-03-22: Added TTS types (TtsRequest, TtsResult, AudioFormat, VoiceProfile, CreateVoiceRequest, CreateVoiceResult, AudioBackend) to audiogen lib.rs.
- 2026-03-22: Created workflow builders: tts.rs (Qwen3-TTS), music.rs (ACE-Step), sfx.rs (Stable Audio).
- 2026-03-22: Added 5 TTS MCP tools: generate_tts, create_voice, list_voices, preview_voice, delete_voice.
- 2026-03-22: Implemented VoiceRegistry with RON persistence under assets/voices/voices.ron.
- 2026-03-22: All 156 tests pass across amigo_comfyui, amigo_artgen, amigo_audiogen. Tool dispatch uses placeholder implementations (no live ComfyUI needed).
