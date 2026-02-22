# Contributing

Thank you for your interest in contributing to uterx! This document provides guidelines and information for contributors.

## Code of Conduct

### Our Pledge

We are committed to providing a welcoming and inclusive environment for all contributors, regardless of:

- Age
- Body size
- Disability
- Ethnicity
- Gender identity and expression
- Level of experience
- Nationality
- Personal appearance
- Race
- Religion
- Sexual identity and orientation

### Our Standards

**Positive behavior includes:**
- Using welcoming and inclusive language
- Being respectful of differing viewpoints and experiences
- Gracefully accepting constructive criticism
- Focusing on what is best for the community
- Showing empathy towards other community members

**Unacceptable behavior includes:**
- The use of sexualized language or imagery
- Trolling, insulting/derogatory comments, or personal/political attacks
- Public or private harassment
- Publishing others' private information without explicit permission
- Any other conduct which could reasonably be considered inappropriate

### Reporting Issues

If you experience or witness unacceptable behavior, please contact the project maintainers.

## Getting Started

### Prerequisites

- Rust 1.92 or later
- Git
- A GitHub account

### Setting Up Development Environment

1. **Fork the repository**:
   ```bash
   # Fork the repository on GitHub
   # Then clone your fork
   git clone https://github.com/yourusername/uterx.git
   cd uterx
   ```

2. **Add upstream remote**:
   ```bash
   git remote add upstream https://github.com/uterx/uterx.git
   ```

3. **Build the project**:
   ```bash
   cargo build --release
   ```

4. **Run tests**:
   ```bash
   cargo test --workspace
   ```

5. **Run linter**:
   ```bash
   cargo clippy --all-targets --all-features -- -D warnings
   ```

6. **Format code**:
   ```bash
   cargo fmt --all
   ```

## Development Workflow

### Branch Strategy

- `main` - Stable release branch
- `develop` - Development branch
- `feature/*` - Feature branches
- `bugfix/*` - Bug fix branches
- `hotfix/*` - Hotfix branches

### Creating a Feature Branch

```bash
# Sync with upstream
git fetch upstream
git checkout develop
git merge upstream/develop

# Create feature branch
git checkout -b feature/my-feature
```

### Making Changes

1. **Write code** following the style guidelines
2. **Add tests** for new functionality
3. **Run tests** to ensure everything passes
4. **Run clippy** and fix any warnings
5. **Format code** with `cargo fmt`

### Committing Changes

Follow conventional commits format:

```
<type>(<scope>): <subject>

<body>

<footer>
```

**Types:**
- `feat` - New feature
- `fix` - Bug fix
- `docs` - Documentation changes
- `style` - Code style changes (formatting, etc.)
- `refactor` - Code refactoring
- `perf` - Performance improvements
- `test` - Test changes
- `chore` - Build process or auxiliary tool changes

**Examples:**

```
feat(mux): add pane reordering with drag-and-drop

Implement drag-and-drop pane reordering for tiled panes.
Users can now drag a pane title bar and drop onto another
pane to swap their positions.

Closes #123
```

```
fix(render): correct cursor position in floating panes

Fixed cursor position calculation for floating panes,
which was previously offset incorrectly.
```

### Pull Request Process

1. **Update your branch**:
   ```bash
   git fetch upstream
   git rebase upstream/develop
   ```

2. **Push to your fork**:
   ```bash
   git push origin feature/my-feature
   ```

3. **Create a Pull Request** on GitHub

4. **Fill in the PR template**:
   - Description of changes
   - Related issues
   - Testing performed
   - Screenshots (if applicable)

5. **Wait for review** and address feedback

### PR Review Guidelines

**Reviewers should:**
- Check that the code follows style guidelines
- Ensure tests are added/updated
- Verify that the change solves the intended problem
- Check for potential bugs or edge cases
- Provide constructive feedback

**Authors should:**
- Respond to review comments promptly
- Make requested changes or discuss alternatives
- Keep the PR focused and small when possible

## Coding Standards

### Rust Style

Follow the official Rust style guidelines:

- Use `cargo fmt` for formatting
- Use `cargo clippy` for linting
- Follow naming conventions:
  - Types: `PascalCase`
  - Functions/Methods: `snake_case`
  - Constants: `SCREAMING_SNAKE_CASE`
  - Modules: `snake_case`

### Documentation

- Add doc comments to all public items
- Use `///` for item documentation
- Use `//!` for module documentation
- Include examples in doc comments

**Example:**

```rust
/// Represents a terminal cell with content and styling.
///
/// # Examples
///
/// ```
/// use uterx_core::{Cell, CellAttributes};
///
/// let cell = Cell::new('A');
/// assert_eq!(cell.c, 'A');
/// ```
pub struct Cell {
    pub c: char,
    pub width: CellWidth,
    pub attrs: CellAttributes,
}
```

### Testing

- Write unit tests for all public functions
- Aim for high code coverage
- Use descriptive test names
- Test edge cases and error conditions

**Example:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cell_creation() {
        let cell = Cell::new('A');
        assert_eq!(cell.c, 'A');
        assert_eq!(cell.width, CellWidth::Single);
    }

    #[test]
    fn test_cell_with_attributes() {
        let attrs = CellAttributes::new();
        let cell = Cell::with_attrs('B', attrs);
        assert_eq!(cell.c, 'B');
    }
}
```

### Error Handling

- Use `Result<T, E>` for fallible operations
- Provide meaningful error messages
- Use `anyhow` for application errors
- Use `thiserror` for library errors

**Example:**

```rust
use anyhow::{Context, Result};

pub fn load_config(path: &Path) -> Result<AppConfig> {
    let content = std::fs::read_to_string(path)
        .context("Failed to read config file")?;

    let config: AppConfig = toml::from_str(&content)
        .context("Failed to parse config file")?;

    Ok(config)
}
```

## Project Structure

```
uterx/
├── crates/              # Internal crates
│   ├── uterx-core/    # Terminal emulation
│   ├── uterx-render/  # GPU rendering
│   ├── uterx-mux/     # Multiplexer
│   ├── uterx-ui/      # TUI layer
│   ├── uterx-plugin/  # Plugin system
│   ├── uterx-platform/# OS abstraction
│   └── uterx-app/     # Main binary
├── plugins/            # Example plugins
├── docs/               # Documentation
├── xtask/             # Build automation
├── Cargo.toml          # Workspace root
└── README.md           # Project README
```

## Areas for Contribution

We welcome contributions in many areas:

### Documentation

- Improve existing documentation
- Add examples and tutorials
- Fix typos and errors
- Translate documentation

### Bug Fixes

- Fix reported issues
- Add tests for bug fixes
- Ensure no regressions

### Features

- Implement new features from the roadmap
- Add support for additional terminal sequences
- Improve performance
- Enhance UI/UX

### Plugins

- Create example plugins
- Improve plugin API
- Add new host functions

### Testing

- Add unit tests
- Add integration tests
- Improve test coverage
- Add benchmarks

## Issue Reporting

### Before Creating an Issue

1. Search existing issues to avoid duplicates
2. Check if the issue is already fixed in the latest version
3. Gather relevant information:
   - uterx version
   - Operating system
   - Steps to reproduce
   - Expected vs actual behavior
   - Logs or screenshots

### Issue Template

```markdown
**Describe the bug**
A clear and concise description of what the bug is.

**To Reproduce**
Steps to reproduce the behavior:
1. Go to '...'
2. Click on '....'
3. Scroll down to '....'
4. See error

**Expected behavior**
A clear and concise description of what you expected to happen.

**Screenshots**
If applicable, add screenshots to help explain your problem.

**Environment**
 - OS: [e.g. Ubuntu 20.04]
 - uterx version: [e.g. 0.1.0]
 - Terminal: [e.g. GNOME Terminal]

**Additional context**
Add any other context about the problem here.
```

## Feature Requests

### Before Requesting a Feature

1. Search existing feature requests
2. Check if the feature aligns with project goals
3. Consider if you can implement it yourself

### Feature Request Template

```markdown
**Is your feature request related to a problem? Please describe.**
A clear and concise description of what the problem is. Ex. I'm always frustrated when [...]

**Describe the solution you'd like**
A clear and concise description of what you want to happen.

**Describe alternatives you've considered**
A clear and concise description of any alternative solutions or features you've considered.

**Additional context**
Add any other context or screenshots about the feature request here.
```

## Getting Help

- **Documentation**: Check the [docs](../README.md) folder
- **Issues**: Search or create an issue on GitHub
- **Discussions**: Use GitHub Discussions for questions
- **Chat**: Join our Discord/Matrix channel (link in README)

## Recognition

Contributors will be recognized in:
- CONTRIBUTORS.md file
- Release notes
- Project README

## License

By contributing to uterx, you agree that your contributions will be licensed under the same license as the project.

## Related Documentation

- [Architecture](./architecture.md) - Project architecture
- [Getting Started](./getting-started.md) - User guide
- [API Reference](./api-reference.md) - API documentation
