//! Covers and mod screenshots, decoded off the UI thread.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Condvar, Mutex};

use crate::steam::game::GameKind;

include!(concat!(env!("OUT_DIR"), "/assets.rs"));

/// Screenshots stay this wide at most; more only costs memory.
const MAX_WIDTH: u32 = 1280;
/// Decoded screenshots kept around. Each is a few MB. Room for every mod's
/// first screenshot plus all of the one in view, so prefetching never evicts
/// what it just loaded.
pub const SCREENSHOT_CACHE_LIMIT: usize = 48;
/// Threads decoding at once, at most.
const MAX_WORKERS: usize = 4;

/// Resource path of a game's Steam header image.
pub fn cover_resource(kind: GameKind) -> &'static str {
    match kind {
        GameKind::SADX => "/io/github/astrovm/AdventureMods/resources/covers/sadx.jpg",
        GameKind::SA2 => "/io/github/astrovm/AdventureMods/resources/covers/sa2.jpg",
    }
}

/// Resource path of the app icon.
pub const APP_ICON: &str = "/io/github/astrovm/AdventureMods/resources/images/app-icon.png";

/// The app icon, for the window and taskbar.
pub fn window_icon() -> egui::IconData {
    let bytes = asset(APP_ICON).expect("the app icon is bundled");
    let image = image::load_from_memory(bytes)
        .expect("the bundled app icon is a valid PNG")
        .to_rgba8();
    egui::IconData {
        width: image.width(),
        height: image.height(),
        rgba: image.into_raw(),
    }
}

/// The bundled bytes of `resource`.
pub fn asset(resource: &str) -> Option<&'static [u8]> {
    ASSETS
        .binary_search_by(|(key, _)| (*key).cmp(resource))
        .ok()
        .map(|index| ASSETS[index].1)
}

/// A bundled image shrunk and blurred into a soft backdrop. Small, so it is
/// cheap to make and smooth when stretched across the window.
pub fn decode_backdrop(resource: &str) -> anyhow::Result<egui::ColorImage> {
    let bytes = asset(resource).ok_or_else(|| anyhow::anyhow!("not bundled"))?;
    let image = image::load_from_memory(bytes)?
        .resize(160, u32::MAX, image::imageops::FilterType::Triangle)
        .blur(5.0)
        .to_rgba8();
    let size = [image.width() as usize, image.height() as usize];
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        image.as_raw(),
    ))
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

/// What the workers should decode next, most wanted first.
#[derive(Default)]
struct Queue {
    pending: Mutex<Pending>,
    added: Condvar,
}

#[derive(Default)]
struct Pending {
    order: VecDeque<&'static str>,
    closed: bool,
}

impl Queue {
    /// The next resource to decode, waiting for one. `None` once closed.
    fn next(&self) -> Option<&'static str> {
        let mut pending = self.pending.lock().unwrap();
        loop {
            if pending.closed {
                return None;
            }
            if let Some(resource) = pending.order.pop_front() {
                return Some(resource);
            }
            pending = self.added.wait(pending).unwrap();
        }
    }
}

/// Textures by resource path. Screenshots decode on worker threads and show
/// up on a later frame; covers are small, so they decode right away.
pub struct ImageCache {
    slots: HashMap<&'static str, Slot>,
    /// Screenshots, least recently used first. Covers are never evicted.
    recent: VecDeque<&'static str>,
    queue: Arc<Queue>,
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
        // Leave a core for the UI.
        let workers = std::thread::available_parallelism()
            .map_or(1, |cores| cores.get() - 1)
            .clamp(1, MAX_WORKERS);
        Self::with_workers(ctx, decode, workers)
    }

    fn with_workers(
        ctx: &egui::Context,
        decode: fn(&str) -> anyhow::Result<egui::ColorImage>,
        workers: usize,
    ) -> Self {
        let (result_tx, results) = std::sync::mpsc::channel();
        let queue = Arc::new(Queue::default());
        for _ in 0..workers {
            let queue = queue.clone();
            let result_tx = result_tx.clone();
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                while let Some(resource) = queue.next() {
                    let _ = result_tx.send((resource, decode(resource)));
                    ctx.request_repaint();
                }
            });
        }
        Self {
            slots: HashMap::new(),
            recent: VecDeque::new(),
            queue,
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
        if is_cover(resource) {
            if !self.slots.contains_key(resource) {
                self.load_now(ctx, resource);
            }
        } else {
            self.touch(resource);
        }
        match self.slots.get(resource) {
            Some(Slot::Ready(texture)) => Some(texture.clone()),
            Some(Slot::Loading | Slot::Failed) => None,
            None => {
                self.slots.insert(resource, Slot::Loading);
                let mut pending = self.queue.pending.lock().unwrap();
                pending.order.push_front(resource);
                self.queue.added.notify_one();
                drop(pending);
                self.evict();
                None
            }
        }
    }

    /// Decode `order` ahead of time, most wanted first, so the screenshots
    /// show up the moment they are needed. Anything else still waiting to be
    /// decoded is dropped, such as when another mod is shown.
    pub fn prefetch(&mut self, ctx: &egui::Context, order: &[&'static str]) {
        self.receive(ctx);
        let order = &order[..order.len().min(SCREENSHOT_CACHE_LIMIT)];
        let mut pending = self.queue.pending.lock().unwrap();
        let queued = std::mem::take(&mut pending.order);
        self.slots
            .retain(|resource, slot| !matches!(slot, Slot::Loading) || order.contains(resource));
        for &resource in order {
            if !self.slots.contains_key(resource) {
                self.slots.insert(resource, Slot::Loading);
                pending.order.push_back(resource);
            } else if queued.contains(&resource) && !pending.order.contains(&resource) {
                pending.order.push_back(resource);
            }
        }
        self.queue.added.notify_all();
        drop(pending);
        // The most wanted ends up the most recently used.
        for &resource in order.iter().rev() {
            self.touch(resource);
        }
        self.evict();
    }

    /// Whether `resource` could not be decoded.
    pub fn failed(&self, resource: &str) -> bool {
        matches!(self.slots.get(resource), Some(Slot::Failed))
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

    fn touch(&mut self, resource: &'static str) {
        self.recent.retain(|cached| *cached != resource);
        self.recent.push_back(resource);
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

    #[cfg(test)]
    pub fn loading(&self) -> usize {
        self.slots
            .values()
            .filter(|slot| matches!(slot, Slot::Loading))
            .count()
    }
}

impl Drop for ImageCache {
    fn drop(&mut self) {
        self.queue.pending.lock().unwrap().closed = true;
        self.queue.added.notify_all();
    }
}

fn is_cover(resource: &str) -> bool {
    resource.contains("/covers/")
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
    fn backdrops_are_small_and_need_a_bundled_image() {
        let backdrop = decode_backdrop(cover_resource(GameKind::SA2)).unwrap();
        assert_eq!(backdrop.size[0], 160);
        assert!(decode_backdrop("/missing.jpg").is_err());
    }

    #[test]
    fn the_window_icon_is_the_square_app_icon() {
        let icon = window_icon();
        assert_eq!((icon.width, icon.height), (256, 256));
        assert_eq!(icon.rgba.len(), 256 * 256 * 4);
        assert!(decode(APP_ICON).is_ok());
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
    fn covers_are_ready_on_the_first_frame() {
        let ctx = egui::Context::default();
        fn never(_: &str) -> anyhow::Result<egui::ColorImage> {
            anyhow::bail!("covers should not wait for a worker")
        }
        let mut cache = ImageCache::with_decoder(&ctx, never);
        let cover = cover_resource(GameKind::SADX);
        let ((), logs) = crate::test_log::capture_logs(|| {
            assert!(cache.get(&ctx, cover).is_none());
        });
        assert!(logs.contains("covers should not wait"), "{logs}");

        let mut cache = ImageCache::new(&ctx);
        assert!(cache.get(&ctx, cover).is_some());
        assert!(cache.get(&ctx, cover).is_some(), "decoded once");
    }

    #[test]
    fn prefetched_screenshots_are_ready_before_they_are_shown() {
        let ctx = egui::Context::default();
        let mut cache = ImageCache::new(&ctx);
        let next = "/io/github/astrovm/AdventureMods/resources/images/sadx/dreamcast_conversion/dreamcast_conversion_after.jpg";
        cache.prefetch(&ctx, &[SCREENSHOT, next, SCREENSHOT]);
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
        while cache.loading() > 0 {
            assert!(std::time::Instant::now() < deadline, "never prefetched");
            std::thread::sleep(std::time::Duration::from_millis(5));
            cache.receive(&ctx);
        }
        assert_eq!(cache.cached(), 2, "each decoded once");
        assert!(cache.get(&ctx, next).is_some(), "no wait when shown");
        assert!(cache.get(&ctx, SCREENSHOT).is_some());

        // Asking again keeps what is decoded.
        cache.prefetch(&ctx, &[next]);
        assert!(matches!(cache.slots.get(next), Some(Slot::Ready(_))));
    }

    #[test]
    fn prefetching_reorders_and_drops_what_is_no_longer_wanted() {
        let ctx = egui::Context::default();
        // No workers, so the queue only changes here.
        let mut cache = ImageCache::with_workers(&ctx, decode, 0);
        let [first, second, third, gone] = ["/a.jpg", "/b.jpg", "/c.jpg", "/d.jpg"];
        cache.prefetch(&ctx, &[first, second, gone]);
        cache.get(&ctx, third);
        let queued = |cache: &ImageCache| -> Vec<&str> {
            cache
                .queue
                .pending
                .lock()
                .unwrap()
                .order
                .iter()
                .copied()
                .collect()
        };
        assert_eq!(queued(&cache), [third, first, second, gone], "shown first");

        cache.prefetch(&ctx, &[second, third, first]);
        assert_eq!(queued(&cache), [second, third, first]);
        assert!(!cache.slots.contains_key(gone), "no longer wanted");
        // A result nobody waits for anymore is dropped.
        cache.store(&ctx, gone, decode(SCREENSHOT));
        assert!(!cache.slots.contains_key(gone));

        // Never more than the cache holds, or it would evict what it loads.
        let many: Vec<&'static str> =
            crate::setup::common::recommended_mods_for_game(GameKind::SADX)
                .iter()
                .flat_map(|mod_entry| mod_entry.pictures.iter().copied())
                .collect();
        assert!(many.len() > SCREENSHOT_CACHE_LIMIT);
        cache.prefetch(&ctx, &many);
        assert_eq!(queued(&cache).len(), SCREENSHOT_CACHE_LIMIT);
        assert_eq!(queued(&cache)[0], many[0]);
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
