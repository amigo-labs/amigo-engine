# Amigo Engine -- Service Architecture

Amigo Engine is a modular, Rust-based 2D game engine designed for AI-assisted development. Its architecture is built around a set of loosely coupled services that communicate through well-defined protocols. Claude Code drives development through MCP (Model Context Protocol) servers that bridge into the engine's JSON-RPC API, while external AI services handle procedural asset and audio generation. The engine itself is organized as a Cargo workspace of focused crates, each owning a single responsibility.

## Service Communication Overview

```mermaid
graph TB
    subgraph "Developer Machine"
        CC["Claude Code<br/>(IDE / CLI)"]

        subgraph "MCP Servers (stdio)"
            MCP["amigo_mcp<br/>MCP to JSON-RPC Bridge"]
            ART["amigo_artgen<br/>Art Generation MCP"]
            AUD["amigo_audiogen<br/>Audio Generation MCP"]
        end

        subgraph "Amigo Engine Process"
            API["amigo_api<br/>JSON-RPC 2.0 Server<br/>(TCP :9999)"]
            ENG["Engine Core<br/>ECS + Renderer + Audio"]
            GAME["Game Code<br/>(Rust)"]
        end

        subgraph "External AI Services"
            COMFY["ComfyUI<br/>(localhost:8188<br/>or Remote GPU)"]
            ACE["ACE-Step<br/>(localhost:7860)"]
            AGEN["AudioGen<br/>(localhost:7861)"]
        end

        subgraph "File System"
            ASSETS["assets/<br/>(Hot Reload)"]
            DATA["data/*.ron<br/>(Configs)"]
            PAK["game.pak<br/>(Release)"]
        end
    end

    subgraph "Multiplayer (Phase 2+)"
        SRV["Game Server<br/>(UDP)"]
        CL2["Client B"]
        CL3["Client C"]
    end

    CC -->|"MCP stdio"| MCP
    CC -->|"MCP stdio"| ART
    CC -->|"MCP stdio"| AUD

    MCP -->|"JSON-RPC TCP"| API
    API -->|"Shared State"| ENG
    ENG -->|"Game Trait"| GAME

    ART -->|"HTTP API"| COMFY
    AUD -->|"HTTP API"| ACE
    AUD -->|"HTTP API"| AGEN

    ART -->|"Generated Assets"| ASSETS
    AUD -->|"Generated Audio"| ASSETS
    ASSETS -->|"Hot Reload"| ENG
    DATA -->|"RON/TOML Config"| ENG

    ENG -->|"UDP (laminar)"| SRV
    SRV -->|"UDP"| CL2
    SRV -->|"UDP"| CL3
```

## Communication Protocols

| Connection | Protocol | Format | Direction |
|-----------|-----------|--------|----------|
| Claude Code <-> MCP Servers | MCP (stdio) | JSON-RPC 2.0 | Bidirectional |
| amigo_mcp <-> amigo_api | TCP Socket | JSON-RPC 2.0 | Bidirectional |
| amigo_artgen <-> ComfyUI | HTTP REST | JSON + Binary | Request/Response |
| amigo_audiogen <-> ACE-Step | HTTP REST | JSON + WAV | Request/Response |
| Engine <-> Assets | Filesystem | PNG/ASE/WAV/RON | Watch + Reload |
| Multiplayer Clients <-> Server | UDP (laminar) | Serialized Commands | Lockstep |

## Internal Engine Architecture

The engine is structured as a set of crates with `amigo_core` at the foundation and `amigo_engine` as the top-level integration crate that pulls everything together.

```mermaid
graph BT
    CORE["amigo_core<br/>Math, ECS, Types"]
    RENDER["amigo_render<br/>wgpu, Camera, Particles"]
    INPUT["amigo_input<br/>Keyboard, Mouse, Gamepad"]
    AUDIO["amigo_audio<br/>kira Wrapper"]
    ASSETS["amigo_assets<br/>Loading, Hot Reload"]
    TILEMAP["amigo_tilemap<br/>Tiles, Auto-tiling"]
    ANIM["amigo_animation<br/>Sprite Animation"]
    SCENE["amigo_scene<br/>State Machine"]
    UI["amigo_ui<br/>Pixel UI"]
    NET["amigo_net<br/>Transport, Replay"]
    DEBUG["amigo_debug<br/>Overlay, Profiling"]
    EDITOR["amigo_editor<br/>Level Editor"]
    API["amigo_api<br/>JSON-RPC IPC"]
    ENGINE["amigo_engine<br/>Game Loop, Builder"]

    RENDER --> CORE
    INPUT --> CORE
    AUDIO --> CORE
    ASSETS --> CORE
    TILEMAP --> CORE
    ANIM --> CORE
    SCENE --> CORE
    UI --> CORE
    NET --> CORE
    DEBUG --> CORE
    EDITOR --> CORE
    API --> CORE

    ENGINE --> RENDER
    ENGINE --> INPUT
    ENGINE --> AUDIO
    ENGINE --> ASSETS
    ENGINE --> TILEMAP
    ENGINE --> ANIM
    ENGINE --> SCENE
    ENGINE --> UI
    ENGINE --> NET
    ENGINE --> DEBUG
```

## Workspace Structure

The full workspace comprises **14 engine crates** (`amigo_core`, `amigo_render`, `amigo_input`, `amigo_audio`, `amigo_assets`, `amigo_tilemap`, `amigo_animation`, `amigo_scene`, `amigo_ui`, `amigo_net`, `amigo_debug`, `amigo_editor`, `amigo_api`, and `amigo_engine`), **4 standalone tools** (`amigo_mcp`, `amigo_artgen`, `amigo_audiogen`, and `amigo_paktool`), and **1 example game** that serves as both a integration test and a reference implementation.
