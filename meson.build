project(
  'adventure-mods',
  'rust',
  version: '0.1.0',
  meson_version: '>= 1.2.0',
  license: 'MIT',
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'io.github.astrovm.AdventureMods'

dependency('glib-2.0', version: '>= 2.76')
dependency('gio-2.0', version: '>= 2.76')
dependency('gtk4', version: '>= 4.14')
dependency('libadwaita-1', version: '>= 1.5')

profile = get_option('profile')
if profile == 'development'
  app_id = '@0@.Devel'.format(base_id)
  vcs_tag = run_command('git', 'rev-parse', '--short', 'HEAD', check: false).stdout().strip()
  if vcs_tag == ''
    version_suffix = '-devel'
  else
    version_suffix = '-@0@'.format(vcs_tag)
  endif
else
  app_id = base_id
  version_suffix = ''
endif

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')
datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
iconsdir = datadir / 'icons'

conf = configuration_data()
conf.set_quoted('APP_ID', app_id)
conf.set_quoted('VERSION', meson.project_version() + version_suffix)
conf.set_quoted('GETTEXT_PACKAGE', 'adventure-mods')
conf.set_quoted('LOCALEDIR', localedir)
conf.set_quoted('PKGDATADIR', pkgdatadir)
conf.set_quoted('PROFILE', profile)

subdir('data')
subdir('po')
subdir('src')
