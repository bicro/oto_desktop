# Contributing to Oto Desktop

Thank you for your interest in contributing to Oto Desktop! This document provides guidelines and information for contributors.

## Getting Started

### Prerequisites

- [Bun](https://bun.sh/) - JavaScript runtime and package manager
- [Rust](https://rustup.rs/) - For building the Tauri backend
- [Tauri CLI](https://tauri.app/v1/guides/getting-started/prerequisites) - Desktop framework

### Development Setup

1. Fork the repository
2. Clone your fork:
   ```bash
   git clone https://github.com/YOUR_USERNAME/oto_desktop.git
   cd oto_desktop
   ```
3. Run the setup script:
   ```bash
   # macOS/Linux
   ./setup.sh

   # Windows
   .\setup.ps1
   ```
4. Start development:
   ```bash
   bun run dev
   ```

## Project Structure

```
oto_desktop/
├── src/                    # Frontend (HTML/JavaScript)
│   ├── index.html         # Main UI
│   └── overlay.html       # Character overlay
├── src-tauri/             # Rust backend
│   ├── src/
│   │   └── main.rs        # Application logic
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
├── lib/                   # Binary dependencies (sqlite-vec)
└── package.json           # Node/Bun configuration
```

## Code Style

### JavaScript

- Use meaningful variable and function names
- Add JSDoc comments for complex functions
- Prefer `const` over `let` when values don't change
- Use template literals for string interpolation

### Rust

- Follow standard Rust conventions (run `cargo fmt`)
- Use `cargo clippy` to catch common issues
- Handle errors properly using `Result` types
- Add documentation comments for public functions

### Pre-commit Checks

Before committing, run the check script to verify code quality:

```bash
# macOS/Linux
./check.sh

# Windows
.\check.ps1
```

This script will:
1. Auto-format Rust code with `cargo fmt`
2. Build the project
3. Run `cargo clippy` with warnings as errors
4. Check for accidentally staged sensitive files (`.api_key`, `.env`, etc.)

## Making Changes

### Branches

- Create a feature branch from `main`:
  ```bash
  git checkout -b feature/your-feature-name
  ```
- Use descriptive branch names:
  - `feature/` - New features
  - `fix/` - Bug fixes
  - `docs/` - Documentation updates
  - `refactor/` - Code refactoring

### Commits

- Write clear, concise commit messages
- Use present tense ("Add feature" not "Added feature")
- Reference issues when applicable: `Fix #123: Description`

### Pull Requests

1. Run the pre-commit check script:
   ```bash
   ./check.sh   # macOS/Linux
   .\check.ps1  # Windows
   ```
2. Update documentation if needed
3. Submit a pull request with:
   - Clear description of changes
   - Screenshots/GIFs for UI changes
   - Reference to related issues

## Testing

Currently, the project doesn't have automated tests. When contributing:

- Test your changes manually on your platform
- Verify the Level 0/1/2 navigation works correctly
- Test both light and dark system themes if applicable
- Check that the character overlay renders properly

## Reporting Issues

When opening an issue, please include:

- Operating system and version
- Steps to reproduce the issue
- Expected vs actual behavior
- Screenshots or error logs if applicable

## Feature Requests

We welcome feature suggestions! Please:

- Check existing issues to avoid duplicates
- Describe the use case clearly
- Explain how it benefits users

## Code of Conduct

- Be respectful and inclusive
- Provide constructive feedback
- Help others learn and grow

## Questions?

If you have questions about contributing, feel free to open a discussion or issue.

Thank you for contributing!
