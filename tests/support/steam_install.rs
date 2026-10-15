//! The file lists of untouched Steam installs, recorded by
//! `build-aux/steam-manifest.sh`. See `tests/fixtures/steam/README.md`.

use std::io::Write;
use std::path::Path;

pub const SADX: &str = include_str!("../fixtures/steam/sadx.tsv");
pub const SA2: &str = include_str!("../fixtures/steam/sa2.tsv");

pub struct GameFile<'a> {
    pub path: &'a str,
    pub size: u64,
    pub sha1: &'a str,
    pub header: Vec<u8>,
}

pub fn files(manifest: &str) -> Vec<GameFile<'_>> {
    manifest
        .lines()
        .filter(|line| !line.starts_with('#'))
        .map(|line| {
            let mut fields = line.split('\t');
            let mut field = || fields.next().expect(line);
            let path = field();
            let size = field().parse().expect(line);
            let sha1 = field();
            let hex = field();
            let header = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).expect(line))
                .collect();
            GameFile {
                path,
                size,
                sha1,
                header,
            }
        })
        .collect()
}

/// Recreate the install under `game_path`: every file with its real name,
/// size and first bytes. The rest of each file is a sparse hole, so this
/// takes no space.
pub fn lay_out(game_path: &Path, manifest: &str) {
    for file in files(manifest) {
        let path = game_path.join(file.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut out = std::fs::File::create(&path).unwrap();
        out.write_all(&file.header).unwrap();
        out.set_len(file.size).unwrap();
    }
}
