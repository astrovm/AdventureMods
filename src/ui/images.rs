//! Covers and mod screenshots, decoded off the UI thread.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::{Arc, Mutex};

use crate::steam::game::GameKind;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// Screenshots stay this wide at most; more only costs memory.
const MAX_WIDTH: u32 = 1280;
/// Decoded screenshots kept around. Each is a few MB.
const SCREENSHOT_CACHE_LIMIT: usize = 24;

/// Resource path of a game's Steam header image.
pub fn cover_resource(kind: GameKind) -> &'static str {
    match kind {
        GameKind::SADX => "/io/github/astrovm/AdventureMods/resources/covers/sadx.jpg",
        GameKind::SA2 => "/io/github/astrovm/AdventureMods/resources/covers/sa2.jpg",
    }
}

/// The bundled bytes of `resource`.
pub fn asset(resource: &str) -> Option<&'static [u8]> {
    ASSETS
        .binary_search_by(|(key, _)| (*key).cmp(resource))
        .ok()
        .map(|index| ASSETS[index].1)
}

/// Decode a bundled image, shrunk to [`MAX_WIDTH`]. Safe on any thread.
pub fn decode(resource: &str) -> anyhow::Result<egui::ColorImage> {
    let bytes = asset(resource).ok_or_else(|| anyhow::anyhow!("not bundled"))?;
    let mut image = image::load_from_memory(bytes)?;
    if image.width() > MAX_WIDTH {
        image = image.resize(MAX_WIDTH, u32::MAX, image::imageops::FilterType::Triangle);
    }
    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        rgba.as_raw(),
    ))
}

enum Slot {
    Loading,
    Ready(egui::TextureHandle),
    Failed,
}

type Decoded = (&'static str, anyhow::Result<egui::ColorImage>);

/// Textures by resource path. Requests decode on a worker thread; the result
/// shows up on a later frame.
pub struct ImageCache {
    slots: HashMap<&'static str, Slot>,
    /// Screenshots, least recently used first. Covers are never evicted.
    recent: VecDeque<&'static str>,
    /// What the worker should still decode; stale requests are skipped.
    wanted: Arc<Mutex<HashSet<&'static str>>>,
    requests: std::sync::mpsc::Sender<&'static str>,
    results: std::sync::mpsc::Receiver<Decoded>,
    decode: fn(&str) -> anyhow::Result<egui::ColorImage>,
}

impl ImageCache {
    pub fn new(ctx: &egui::Context) -> Self {
        Self::with_decoder(ctx, decode)
    }

    pub fn with_decoder(
        ctx: &egui::Context,
        decode: fn(&str) -> anyhow::Result<egui::ColorImage>,
    ) -> Self {
        let (requests, request_rx) = std::sync::mpsc::channel::<&'static str>();
        let (result_tx, results) = std::sync::mpsc::channel();
        let wanted = Arc::new(Mutex::new(HashSet::new()));
        let worker_wanted = wanted.clone();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            for resource in request_rx {
                if !worker_wanted.lock().unwrap().remove(resource) {
                    continue;
                }
                let _ = result_tx.send((resource, decode(resource)));
                ctx.request_repaint();
            }
        });
        Self {
            slots: HashMap::new(),
            recent: VecDeque::new(),
            wanted,
            requests,
            results,
            decode,
        }
    }

    /// The texture for `resource`, or `None` while it decodes or if it failed.
    pub fn get(
        &mut self,
        ctx: &egui::Context,
        resource: &'static str,
    ) -> Option<egui::TextureHandle> {
        self.receive(ctx);
        if !resource.contains("/covers/") {
            self.recent.retain(|cached| *cached != resource);
            self.recent.push_back(resource);
        }
        match self.slots.get(resource) {
            Some(Slot::Ready(texture)) => Some(texture.clone()),
            Some(Slot::Loading | Slot::Failed) => None,
            None => {
                self.slots.insert(resource, Slot::Loading);
                self.wanted.lock().unwrap().insert(resource);
                let _ = self.requests.send(resource);
                self.evict();
                None
            }
        }
    }

    /// Whether `resource` could not be decoded.
    pub fn failed(&self, resource: &str) -> bool {
        matches!(self.slots.get(resource), Some(Slot::Failed))
    }

    /// Stop decoding everything except `keep`, such as when another mod is shown.
    pub fn retain_pending(&mut self, keep: &[&'static str]) {
        self.wanted
            .lock()
            .unwrap()
            .retain(|resource| keep.contains(resource));
        self.slots
            .retain(|resource, slot| !matches!(slot, Slot::Loading) || keep.contains(resource));
    }

    /// Decode `resource` right away on this thread.
    pub fn load_now(&mut self, ctx: &egui::Context, resource: &'static str) {
        let result = (self.decode)(resource);
        self.slots.insert(resource, Slot::Loading);
        self.store(ctx, resource, result);
    }

    fn receive(&mut self, ctx: &egui::Context) {
        while let Ok((resource, result)) = self.results.try_recv() {
            self.store(ctx, resource, result);
        }
    }

    fn store(
        &mut self,
        ctx: &egui::Context,
        resource: &'static str,
        result: anyhow::Result<egui::ColorImage>,
    ) {
        // Dropped while decoding: nobody waits for it anymore.
        if !self.slots.contains_key(resource) {
            return;
        }
        let slot = match result {
            Ok(image) => {
                Slot::Ready(ctx.load_texture(resource, image, egui::TextureOptions::LINEAR))
            }
            Err(err) => {
                tracing::warn!("Failed to load image {resource}: {err}");
                Slot::Failed
            }
        };
        self.slots.insert(resource, slot);
    }

    fn evict(&mut self) {
        let excess = self.recent.len().saturating_sub(SCREENSHOT_CACHE_LIMIT);
        for evicted in self.recent.drain(..excess) {
            self.slots.remove(evicted);
        }
    }

    #[cfg(test)]
    pub fn cached(&self) -> usize {
        self.slots.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREENSHOT: &str = "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_conversion/dreamcast_conversion_before.jpg";

    /// Poll until `resource` decoded, failing after a few seconds.
    pub(crate) fn wait_for(
        cache: &mut ImageCache,
        ctx: &egui::Context,
        resource: &'static str,
    ) -> Option<egui::TextureHandle> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        loop {
            if let Some(texture) = cache.get(ctx, resource) {
                return Some(texture);
            }
            if cache.failed(resource) {
                return None;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "{resource} never loaded"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    #[test]
    fn every_catalog_picture_and_cover_is_bundled() {
        for kind in [GameKind::SADX, GameKind::SA2] {
            assert!(asset(cover_resource(kind)).is_some());
            for mod_entry in crate::setup::common::recommended_mods_for_game(kind) {
                for picture in mod_entry.pictures {
                    assert!(asset(picture).is_some(), "{picture} is not bundled");
                }
            }
        }
        assert!(asset("/missing.jpg").is_none());
    }

    #[test]
    fn large_screenshots_are_shrunk_when_decoded() {
        let image = decode(SCREENSHOT).unwrap();
        assert!(image.size[0] <= MAX_WIDTH as usize);
        assert!(image.size[1] > 0);

        let cover = decode(cover_resource(GameKind::SA2)).unwrap();
        assert_eq!(cover.size, [460, 215]);
        assert!(decode("/missing.jpg").is_err());
    }

    #[test]
    fn textures_decode_in_the_background_and_report_failures() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::new(&ctx);

        assert!(
            cache.get(&ctx, SCREENSHOT).is_none(),
            "decoding takes a while"
        );
        assert!(wait_for(&mut cache, &ctx, SCREENSHOT).is_some());
        assert!(cache.get(&ctx, SCREENSHOT).is_some(), "now cached");

        let ((), logs) = crate::test_log::capture_logs(|| {
            assert!(wait_for(&mut cache, &ctx, "/missing.jpg").is_none());
        });
        assert!(logs.contains("Failed to load image /missing.jpg"), "{logs}");
        assert!(
            cache.get(&ctx, "/missing.jpg").is_none(),
            "failures are not retried"
        );
    }

    #[test]
    fn stale_requests_are_dropped_and_covers_load_now() {
        let ctx = egui::Context::default();
        fn slow(resource: &str) -> anyhow::Result<egui::ColorImage> {
            std::thread::sleep(std::time::Duration::from_millis(50));
            decode(resource)
        }
        let mut cache = ImageCache::with_decoder(&ctx, slow);
        let busy = "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_conversion/dreamcast_conversion_after.jpg";
        let skipped =
            "/io/github/astrovm/AdventureMods/resources/images/sa2/hd_gui/hdguiforsa2_0.jpg";
        let cover = cover_resource(GameKind::SADX);

        // The worker is busy with the first while the others queue up.
        cache.get(&ctx, busy);
        cache.get(&ctx, skipped);
        cache.get(&ctx, SCREENSHOT);
        cache.retain_pending(&[SCREENSHOT]);
        assert!(wait_for(&mut cache, &ctx, SCREENSHOT).is_some());
        std::thread::sleep(std::time::Duration::from_millis(100));
        cache.receive(&ctx);
        assert!(
            !cache.slots.contains_key(busy),
            "finished, but no longer wanted"
        );
        assert!(!cache.slots.contains_key(skipped), "never decoded");
        // However the worker raced, a result nobody asked for is dropped.
        cache.store(&ctx, skipped, decode(SCREENSHOT));
        assert!(!cache.slots.contains_key(skipped));

        cache.load_now(&ctx, cover);
        assert!(cache.get(&ctx, cover).is_some());
    }

    #[test]
    fn the_screenshot_cache_is_bounded() {
        let ctx = egui::Context::default();
        fn tiny(_: &str) -> anyhow::Result<egui::ColorImage> {
            Ok(egui::ColorImage::filled([1, 1], egui::Color32::BLACK))
        }
        let mut cache = ImageCache::with_decoder(&ctx, tiny);
        let mods = crate::setup::common::recommended_mods_for_game(GameKind::SA2);
        let pictures: Vec<&'static str> = mods
            .iter()
            .flat_map(|mod_entry| mod_entry.pictures.iter().copied())
            .collect();
        assert!(pictures.len() > SCREENSHOT_CACHE_LIMIT);

        for picture in &pictures {
            cache.get(&ctx, picture);
        }
        cache.get(&ctx, cover_resource(GameKind::SA2));

        assert_eq!(cache.cached(), SCREENSHOT_CACHE_LIMIT + 1);
    }

    #[test]
    fn the_worker_stops_when_the_cache_is_dropped() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::new(&ctx);
        cache.get(&ctx, SCREENSHOT);
        drop(cache);
    }
}
