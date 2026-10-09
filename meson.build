project(
  'adventure-mods',
  'rust',
  version: run_command('sh', 'build-aux/cargo-version.sh', check: true).stdout().strip(),
  meson_version: '>= 1.2.0',
  license: 'MIT',
)

base_id = 'io.github.astrovm.AdventureMods'

# Controller support reads devices through libudev.
dependency('libudev')

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
datadir = prefix / get_option('datadir')
iconsdir = datadir / 'icons'

conf = configuration_data()
conf.set('APP_ID_RAW', app_id)
conf.set('APP_NAME_RAW', app_name)
conf.set_quoted('APP_ID', app_id)
conf.set_quoted('APP_NAME', app_name)
conf.set_quoted('PROFILE', profile)

subdir('data')
subdir('src')
