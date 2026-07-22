configure_file(
  input: 'config.rs.in',
  output: 'config.rs',
  configuration: conf,
)

run_target(
  'cargo-build',
  command: [
    find_program(meson.project_source_root() / 'build-aux' / 'cargo-build.sh'),
    meson.project_build_root(),
    meson.project_source_root(),
    '@OUTPUT@',
    get_option('buildtype'),
    'adventure-mods',
  ],
)

cargo_build = custom_target(
  'cargo-build',
  build_by_default: true,
  build_always_stale: true,
  output: meson.project_name(),
  console: true,
  install: true,
  install_dir: bindir,
  command: [
    find_program(meson.project_source_root() / 'build-aux' / 'cargo-build.sh'),
    meson.project_build_root(),
    meson.project_source_root(),
    '@OUTPUT@',
    get_option('buildtype'),
    meson.project_name(),
  ],
)

cargo_test_env = environment()
cargo_test_env.set('CARGO_NET_OFFLINE', 'true')
cargo_test_env.set('CARGO_TARGET_DIR', meson.project_build_root() / 'target')

test(
  'Rust tests',
  find_program('cargo'),
  args: [
    'test',
    '--locked',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
  ],
  depends: cargo_build,
  env: cargo_test_env,
  timeout: 600,
)
