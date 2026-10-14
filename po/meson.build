i18n = import('i18n')

# Rust has no gettext preset in meson, so pass the keywords the code uses.
i18n.gettext(
  meson.project_name(),
  preset: 'glib',
  args: ['--language=C', '--keyword=tr', '--keyword=trn:1,2', '--keyword=gettext', '--keyword=ngettext:1,2', '--keyword=tr_fmt:1', '--keyword=mark'],
)
