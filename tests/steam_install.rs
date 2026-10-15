mod support;

use std::io::Read;
use std::path::Path;

use support::steam_install::{self, GameFile, SA2, SADX};

#[test]
fn manifests_list_each_file_once_in_order() {
    for manifest in [SADX, SA2] {
        let files = steam_install::files(manifest);
        assert!(files.len() > 4000);
        for pair in files.windows(2) {
            assert!(pair[0].path < pair[1].path, "{}", pair[1].path);
        }
        for file in &files {
            assert_eq!(file.sha1.len(), 40, "{}", file.path);
            assert!(file.sha1.bytes().all(|b| b.is_ascii_hexdigit()));
            assert_eq!(file.header.len() as u64, file.size.min(8), "{}", file.path);
        }
    }
}

#[test]
fn steam_sadx_already_ships_adx_audio() {
    let files = steam_install::files(SADX);
    let extension = |file: &GameFile| file.path.rsplit('.').next().unwrap().to_ascii_lowercase();
    assert!(files.iter().all(|file| extension(file) != "wma"));

    // The music and voices sit in folders named "wma", but every file in
    // them is ADX: the extension says so and the header starts with 0x80 0x00.
    let audio: Vec<_> = files
        .iter()
        .filter(|file| {
            let path = file.path.to_ascii_lowercase();
            ["bgm/", "voice_jp/", "voice_us/"]
                .iter()
                .any(|dir| path.starts_with(&format!("sounddata/{dir}")))
        })
        .collect();
    assert_eq!(audio.len(), 121 + 2049 + 2049);
    for file in audio {
        assert!(
            file.path.to_ascii_lowercase().contains("/wma/"),
            "{}",
            file.path
        );
        assert_eq!(extension(file), "adx", "{}", file.path);
        assert_eq!(file.header[..2], [0x80, 0x00], "{}", file.path);
    }
}

#[test]
fn lay_out_recreates_names_sizes_and_headers() {
    let dir = tempfile::tempdir().unwrap();
    steam_install::lay_out(dir.path(), SA2);

    let mut found = Vec::new();
    walk(dir.path(), dir.path(), &mut found);
    found.sort();
    let files = steam_install::files(SA2);
    assert_eq!(
        found,
        files.iter().map(|file| file.path).collect::<Vec<_>>()
    );

    for file in files {
        let path = dir.path().join(file.path);
        assert_eq!(std::fs::metadata(&path).unwrap().len(), file.size);
        let mut header = Vec::new();
        std::fs::File::open(&path)
            .unwrap()
            .take(8)
            .read_to_end(&mut header)
            .unwrap();
        assert_eq!(header, file.header, "{}", file.path);
    }
}

fn walk(root: &Path, dir: &Path, found: &mut Vec<String>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            walk(root, &path, found);
        } else {
            let relative = path.strip_prefix(root).unwrap();
            found.push(relative.to_str().unwrap().to_owned());
        }
    }
}
