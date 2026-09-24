project(
  'adventure-mods',
  'rust',
  version: run_command('sh', 'build-aux/cargo-version.sh', check: true).stdout().strip(),
  meson_version: '>= 1.2.0',
  license: 'MIT',
)

i18n = import('i18n')
gnome = import('gnome')

base_id = 'io.github.astrovm.AdventureMods'

dependency('glib-2.0', version: '>= 2.84')
dependency('gio-2.0', version: '>= 2.84')
dependency('gtk4', version: '>= 4.22')
dependency('libadwaita-1', version: '>= 1.9')

profile = get_option('profile')
if profile == 'development'
  app_id = '@0@.Devel'.format(base_id)
  app_name = 'Adventure Mods Devel'
else
  app_id = base_id
  app_name = 'Adventure Mods'
endif

prefix = get_option('prefix')
bindir = prefix / get_option('bindir')
localedir = prefix / get_option('localedir')
datadir = prefix / get_option('datadir')
pkgdatadir = datadir / meson.project_name()
iconsdir = datadir / 'icons'

conf = configuration_data()
conf.set('APP_ID_RAW', app_id)
conf.set('APP_PATH_RAW', '/@0@/'.format(app_id.replace('.', '/')))
conf.set('APP_NAME_RAW', app_name)
conf.set_quoted('APP_ID', app_id)
conf.set_quoted('APP_PATH', '/@0@/'.format(app_id.replace('.', '/')))
conf.set_quoted('APP_NAME', app_name)
conf.set_quoted('GETTEXT_PACKAGE', 'adventure-mods')
conf.set_quoted('LOCALEDIR', localedir)
conf.set_quoted('PKGDATADIR', pkgdatadir)
conf.set_quoted('PROFILE', profile)

subdir('data')
subdir('po')
subdir('src')
