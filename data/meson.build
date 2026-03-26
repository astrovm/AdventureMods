configured_desktop = configure_file(
  input: '@0@.desktop.in'.format(base_id),
  output: '@0@.desktop.in'.format(app_id),
  configuration: conf,
)

desktop_file = i18n.merge_file(
  input: configured_desktop,
  output: '@0@.desktop'.format(app_id),
  type: 'desktop',
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: datadir / 'applications',
)

desktop_utils = find_program('desktop-file-validate', required: false)
if desktop_utils.found()
  test('Validate desktop file', desktop_utils, args: [desktop_file])
endif

configured_metainfo = configure_file(
  input: '@0@.metainfo.xml.in'.format(base_id),
  output: '@0@.metainfo.xml.in'.format(app_id),
  configuration: conf,
)

metainfo_file = i18n.merge_file(
  input: configured_metainfo,
  output: '@0@.metainfo.xml'.format(app_id),
  po_dir: meson.project_source_root() / 'po',
  install: true,
  install_dir: datadir / 'metainfo',
)

appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
  test('Validate metainfo file', appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file],
  )
endif

configured_schema = configure_file(
  input: '@0@.gschema.xml'.format(base_id),
  output: '@0@.gschema.xml'.format(app_id),
  configuration: conf,
)

install_data(
  configured_schema,
  install_dir: datadir / 'glib-2.0' / 'schemas',
)

compile_schemas = find_program('glib-compile-schemas', required: false)
if compile_schemas.found()
  test('Validate schema file',
    compile_schemas,
    args: ['--strict', '--dry-run', meson.current_source_dir()],
  )
endif

install_data(
  '@0@.svg'.format(base_id),
  install_dir: iconsdir / 'hicolor' / 'scalable' / 'apps',
  rename: '@0@.svg'.format(app_id),
)

gnome.compile_resources(
  'adventure-mods',
  'resources.gresource.xml',
  gresource_bundle: true,
  install: true,
  install_dir: pkgdatadir,
)
