# Oto Desktop

A desktop Live2D character companion with AI chat integration. Oto Desktop creates a transparent overlay with an animated character that can see your screen, have conversations with you, and provide AI-assisted responses.

## Features

- **Live2D Character Overlay**: Animated character rendered using Live2D Cubism SDK
- **AI Chat Integration**: Powered by OpenAI GPT models for intelligent conversations
- **Screen Awareness**: Optionally capture screenshots to give the AI context about what you're working on
- **Three Conversation Levels**:
  - **Level 0 (Standard)**: General AI assistant with character commentary
  - **Level 1 (Character)**: Direct dialogue with the character personality
  - **Level 2 (Deep Think)**: Deep analysis and reflection mode
- **Cross-Platform**: Works on macOS, Windows, and Linux
- **Global Shortcuts**: Quick access keyboard shortcuts
- **Head Tracking**: Character follows your cursor movement

## Installation

### Prerequisites

- [Bun](https://bun.sh/) (JavaScript runtime)
- [Rust](https://rustup.rs/) (for building the Tauri backend)
- OpenAI API key

### macOS / Linux

```bash
./setup.sh
bun run dev
```

### Windows

```powershell
Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
.\setup.ps1
bun run dev
```

## Building

```bash
# macOS (Universal binary)
bun run build:mac

# Windows
bun run build:windows

# Linux
bun run build
```

## Development

Before committing changes, run the pre-commit check script:

```bash
./check.sh   # macOS/Linux
.\check.ps1  # Windows
```

This formats code, builds, runs clippy, and checks for sensitive files.

## Usage

### Initial Setup

1. Launch Oto Desktop
2. Enter your OpenAI API key when prompted
3. The character will appear as a transparent overlay

### Keyboard Navigation

| Key | Action |
|-----|--------|
| `Arrow Left/Right` | Switch between conversation levels (0, 1, 2) |
| `Arrow Up/Down` | Navigate through chat history |
| `Enter` | Send message (or trigger Deep Think at Level 2) |
| `Escape` | Close history modal |

### Conversation Levels

**Level 0 - Standard Mode**
- Chat with an AI assistant
- Character provides additional commentary
- Screenshots can be attached for context

**Level 1 - Character Mode**
- Direct conversation with the character personality
- More personal and conversational tone

**Level 2 - Deep Think Mode**
- Analytical and reflective responses
- 6-hour cooldown between deep analysis sessions
- Press Enter with empty input to trigger deep research

## Platform Notes

### macOS

In **System Settings > Privacy & Security**, enable:
- **Accessibility**: Required for global shortcuts (enable for Terminal/IDE)
- **Screen Recording**: Required for screenshot capture (enable for Oto Desktop)

### Windows

Run the setup script in PowerShell with appropriate execution policy.

### Linux

Ensure you have the required system libraries for Tauri applications.

## Privacy Notice

Oto Desktop includes features that access system-level data:

1. **Screen Capture**: When enabled, screenshots are taken and sent to OpenAI's API to provide context-aware responses. Screenshots are stored locally and not shared except with the configured AI provider.

2. **Mouse Position Tracking**: Used only for character head-tracking animation. This data is not stored or transmitted.

3. **Chat History**: Conversations are stored locally in a SQLite database on your device.

4. **API Key Storage**: Your OpenAI API key is stored locally in plaintext. Keep your system secure.

All AI processing is done via OpenAI's API. Please review [OpenAI's privacy policy](https://openai.com/policies/privacy-policy) for information about how they handle data.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

### Third-Party Licenses

This project uses the Live2D Cubism SDK, which has its own licensing terms:
- **Individuals and small businesses**: Free to use
- **Commercial/enterprise use**: May require a license agreement

Please review the [Live2D SDK License](https://www.live2d.com/en/sdk/license/) before distributing applications built with this project.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines on how to contribute to this project.

## Acknowledgments

- [Live2D Inc.](https://www.live2d.com/) for the Cubism SDK
- [Tauri](https://tauri.app/) for the desktop framework
- [Pixi.js](https://pixijs.com/) for 2D rendering
- [pixi-live2d-display](https://github.com/guansss/pixi-live2d-display) for Live2D integration
