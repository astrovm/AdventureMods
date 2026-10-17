use std::os::unix::fs::PermissionsExt;
use std::path::Path;

pub fn write_script(path: &Path, content: &str) {
    std::fs::write(path, content).unwrap();
    let mut permissions = std::fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(path, permissions).unwrap();
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

/// A fake 7zz that fails on every archive except the .NET zips, which it
/// unpacks like [`install_fake_7zz`].
pub fn install_fake_7zz_failing_all_but_dotnet(path: &Path, extract_root: &Path) {
    let dotnet_7zz = path.with_file_name("fake-7zz-dotnet");
    install_fake_7zz(&dotnet_7zz, extract_root);
    write_script(
        path,
        &format!(
            "#!/bin/sh\ncase \"$*\" in *-runtime-*-win-x64.zip) exec \"{}\" \"$@\" ;; esac\nexit 1\n",
            dotnet_7zz.display()
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
