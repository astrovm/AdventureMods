use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub fn write_script(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
}

pub fn install_fake_wine(path: &Path, log_path: &Path) {
    write_script(
        path,
        &format!(
            "#!/bin/sh\nset -eu\nprintf '%s\\n' \"$@\" > \"{}\"\nmkdir -p \"$WINEPREFIX/drive_c/Program Files/dotnet/shared/Microsoft.WindowsDesktop.App/8.0.0\"\n",
            log_path.display()
        ),
    );
}

pub fn install_fake_7zz(path: &Path, extract_root: &Path) {
    write_script(
        path,
        &format!(
            "#!/bin/sh\nset -eu\ndest=''\narchive=''\nfor arg in \"$@\"; do\n  case \"$arg\" in\n    -o*) dest=${{arg#-o}} ;;\n    x|-y) ;;\n    *) archive=\"$arg\" ;;\n  esac\ndone\nkey=$(tr -d '\\r\\n' < \"$archive\")\nsrc=\"{}/$key\"\nmkdir -p \"$dest\"\ncp -R \"$src\"/. \"$dest\"/\n",
            extract_root.display()
        ),
    );
}

pub fn install_fake_hpatchz(path: &Path, patched_root: &Path) {
    write_script(
        path,
        &format!(
            "#!/bin/sh\nset -eu\nout_dir=\"$4\"\nmkdir -p \"$out_dir\"\ncp -R \"{}\"/. \"$out_dir\"/\n",
            patched_root.display()
        ),
    );
}
