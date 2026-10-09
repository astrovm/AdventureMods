desktop_file = configure_file(
  input: '@0@.desktop.in'.format(base_id),
  output: '@0@.desktop'.format(app_id),
  configuration: conf,
  install: true,
  install_dir: datadir / 'applications',
)

desktop_utils = find_program('desktop-file-validate', required: false)
if desktop_utils.found()
  test('Validate desktop file', desktop_utils, args: [desktop_file])
endif

metainfo_file = configure_file(
  input: '@0@.metainfo.xml.in'.format(base_id),
  output: '@0@.metainfo.xml'.format(app_id),
  configuration: conf,
  install: true,
  install_dir: datadir / 'metainfo',
)

appstreamcli = find_program('appstreamcli', required: false)
if appstreamcli.found()
  test('Validate metainfo file', appstreamcli,
    args: ['validate', '--no-net', '--explain', metainfo_file],
  )
endif

install_data(
  '@0@.svg'.format(base_id),
  install_dir: iconsdir / 'hicolor' / 'scalable' / 'apps',
  rename: '@0@.svg'.format(app_id),
)
