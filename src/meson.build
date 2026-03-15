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
