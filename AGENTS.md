# Repository Guidelines

## Project Structure & Module Organization
`src/` contains the Rust application code. Keep GTK/libadwaita UI code in `src/ui/`, setup flow and game-specific logic in `src/setup/`, Steam parsing in `src/steam/`, and download/archive/protontricks integrations in `src/external/`. Meson generates `src/config.rs` from `src/config.rs.in`, so edit the template, not the generated file. Packaging assets live in `data/`, translations in `po/`, and Flatpak/Meson helpers in `build-aux/`.

## Build, Test, and Development Commands
Install the GTK4/libadwaita/Meson development packages from `README.md` first; otherwise `cargo` will fail at `pkg-config` checks for libraries such as `gtk4`, `pango`, and `gdk-pixbuf-2.0`.

- `cargo build`: fast local development build.
- `cargo test`: runs the inline unit tests spread across `src/**`.
- `cargo fmt --all`: formats the Rust codebase with standard `rustfmt` rules.
- `cargo clippy --all-targets`: catches common Rust issues before review.
- `meson setup builddir && meson compile -C builddir`: builds the desktop app through Meson.
- `flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.json`: produces the Flatpak build described in the README.

## Coding Style & Naming Conventions
Follow default Rust formatting: 4-space indentation, trailing commas where `rustfmt` adds them, and grouped `use` statements similar to `src/ui/setup_page.rs`. Use `snake_case` for modules, files, and functions, `PascalCase` for types and GTK widget wrappers, and `SCREAMING_SNAKE_CASE` for constants. Prefer small, focused modules and keep game-specific behavior in `sadx*` or `sa2*` files rather than branching throughout shared code.

## Testing Guidelines
Tests are colocated with implementation under `#[cfg(test)] mod tests`. Add coverage next to the changed code instead of creating ad hoc test folders. Favor deterministic unit tests for config generation, VDF parsing, step sequencing, and download/setup helpers. Run `cargo test` before opening a PR; use `cargo test setup::sadx` or a similar path filter when iterating.

## Commit & Pull Request Guidelines
Recent history uses Conventional Commits such as `feat:`, `refactor:`, and `chore:`. Keep subjects imperative and specific, for example `feat: add SA2 patch defaults`. PRs should explain user-visible behavior, list verification commands run, and include screenshots for UI changes. Link any related issue and call out packaging or Flatpak impact when relevant.

## Security & Configuration Tips
Do not commit local build outputs from `target/`, `build/`, or extraction directories. Treat external download URLs, archive handling, and `flatpak-spawn --host` integrations as sensitive paths: validate changes carefully and avoid broad filesystem or host-command access.
