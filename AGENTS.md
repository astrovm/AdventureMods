## Build & run

```sh
# Flatpak (GUI development)
flatpak-builder --user --install build build-aux/io.github.astrovm.AdventureMods.Devel.json

# AppImage (release builds, uses Podman + debian:13)
# See README.md for the full podman run command; outputs to appimage-build/
```

There is no plain `cargo build` workflow — the binary needs Meson-generated `config.rs` and compiled GResources to run. Use Flatpak for GUI dev, Podman for AppImage builds.

## Check commands

```sh
cargo fmt --check                              # formatting
cargo clippy --all-targets -- -D warnings       # lint (warnings are errors)
xvfb-run cargo test                            # tests (GTK needs a display)
```

CI runs all three in that order on ubuntu:26.04 (see `.github/workflows/tests.yml`). Tests require system packages `libgtk-4-dev`, `libadwaita-1-dev`, and `xvfb` because the binary links GTK at compile time.

## Project structure

Rust GTK4/libadwaita app with Meson build system. Meson generates `src/config.rs` from `src/config.rs.in` and compiles GResources from `data/resources.gresource.xml`. The Cargo build is wrapped by `build-aux/cargo-build.sh` invoked as a Meson custom target.

```
src/main.rs          # GUI entrypoint; falls back to CLI via cli::run_from_args
src/cli.rs           # CLI: detect, list-mods, setup subcommands
src/application.rs   # GTK AdwApplication
src/ui/              # GTK widgets (welcome_page, setup_page, game_card)
src/setup/           # Mod installation pipeline (sadx.rs, sa2.rs, pipeline.rs)
src/steam/           # Steam library detection, VDF parsing, game identification
src/external/        # Downloads, archive extraction, Proton integration, runtime installer
src/config.rs.in     # Template → Meson generates src/config.rs with APP_ID, PKGDATADIR, etc.
data/                # Desktop file, metainfo XML, GSettings schema, icons, GResources
build-aux/appimage/  # AppImage build script + AppRun hooks
tests/               # Integration tests (e2e_sadx, e2e_sa2, download, cli)
```

The CLI shares the same binary as the GUI. `main.rs` checks `cli::looks_like_cli()` — if args contain a known subcommand it runs CLI mode, otherwise it launches the GTK GUI. `cli::run_from_args` returns `Ok(false)` to signal "launch GUI instead."

## Version bumps

Version is defined in `Cargo.toml` only. `Cargo.lock` and `meson.build` read it from there. On release, also add a new `<release>` entry to `data/io.github.astrovm.AdventureMods.metainfo.xml.in` with today's date.

The AppImage CI (`.github/workflows/appimage.yml`) extracts release notes from that metainfo file via `build-aux/extract-release-notes.py`. Pushing a `v*` tag triggers the build and creates a draft GitHub Release.

## Testing notes

- Tests use `with_test_settings()` which compiles GSettings schemas into a temp dir and sets `GSETTINGS_SCHEMA_DIR` — this requires `glib-compile-schemas` on PATH.
- Tests that touch GTK need a display server (`xvfb-run` in CI, or a running Wayland/X11 session locally).
- E2E tests (`tests/e2e_sadx.rs`, `tests/e2e_sa2.rs`) hit the network (GameBanana, GitHub) — they may fail without internet or if upstream URLs change.

## AppImage packaging

Built inside `debian:13` container. Source-builds GTK4 and libadwaita for the latest version. Excludes `libvulkan.so.*` and `libwayland-egl.so.*` (must use host Mesa drivers) but bundles libxml2. Auto-updates via zsync. See `build-aux/appimage/build-appimage.sh`.

## Approach

- Read before editing. Test before declaring done.
- Prefer small edits over rewrites.
- Reproduce before fixing runtime or external issues.
- Unproven concerns are risks, not bugs. Say so if not reproduced.
- Simplest working solution. No over-engineering, speculative features, or single-use abstractions.

## Output

- Code first. Explain only non-obvious logic.
- No filler, boilerplate, or out-of-scope suggestions.

## Code

- Remove unused imports, variables, parameters, dead branches, and dead functions from edited files.
- No error handling for impossible scenarios.
- All imports at top of file. None inside functions unless strictly required to break circular dependencies.
- Code and comments in English. User-facing strings stay in their original language.

## Maintenance

- Remove old code when introducing replacements. No backward compatibility shims without explicit authorization.
- Do not preserve feature flags for shipped features or abstractions that serve a single caller.

## Debugging

- Read code before explaining. Prove with direct evidence: failing test, reproduced run, or concrete probe.
- State what you found, where, and the fix. If unclear, say so.

## Verification

- Smallest proof first, then broader checks.
- Use the standard toolchain. Default checks: format, lint (warnings as errors), tests. Skip only with stated reason.
- No "fixed/safe/ready" claims without fresh command output.
- Fix every issue you encounter. There are no pre-existing bugs or errors to ignore.

## Tests

- If the project has tests, run them before committing or declaring work complete. No exceptions.
- A failing test is a blocking issue. Fix it before moving on.

## Git

- Ask before pushing every time, even if previously approved.
- No batch commit+push. No force push or hard reset without approval.
- Never `git commit --amend` unless explicitly asked.
- Merge to `main` with a single squashed commit. Commit messages in English.

## Configuration

- Environment variables only for secrets and external credentials.
- Prioritize sane defaults, zero-config, and easy maintenance. Hardcode sensible defaults for internal URLs, ports, and feature flags.
- When adding a dependency, verify the actual latest version from the registry or official source. Never rely on model memory.

## Formatting

- Plain hyphens and straight quotes only. No decorative Unicode. Code output copy-paste safe.
