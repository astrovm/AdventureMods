use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::blocking;
use crate::setup::{common, config, pipeline, sadx, steps};
use crate::steam::game::Game;

const MOD_PREVIEW_IMAGE_HEIGHT: i32 = 250;
const MOD_PREVIEW_DESCRIPTION_HEIGHT: i32 = 150;
mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/setup_page.ui")]
    pub struct AdventureModsSetupPage {
        #[template_child]
        pub body_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub step_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub step_description: TemplateChild<gtk::Label>,
        #[template_child]
        pub content_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub next_button: TemplateChild<gtk::Button>,

        pub game: RefCell<Option<Game>>,
        pub current_step: Cell<usize>,
        pub all_steps: RefCell<Vec<steps::SetupStep>>,
        pub selected_mods: RefCell<Vec<usize>>,
        pub language_selection: RefCell<Option<config::LanguageSelection>>,
        pub cancel_flag: RefCell<Option<Arc<AtomicBool>>>,
        pub is_error: Cell<bool>,
    }

    impl std::fmt::Debug for AdventureModsSetupPage {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AdventureModsSetupPage").finish()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsSetupPage {
        const NAME: &'static str = "AdventureModsSetupPage";
        type Type = super::AdventureModsSetupPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AdventureModsSetupPage {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj().clone();
            self.next_button.connect_clicked(move |_| {
                obj.on_next_clicked();
            });

            let obj = self.obj().clone();
            self.back_button.connect_clicked(move |_| {
                obj.on_back_clicked();
            });
        }
    }

    impl WidgetImpl for AdventureModsSetupPage {}
    impl BinImpl for AdventureModsSetupPage {}
}

glib::wrapper! {
    pub struct AdventureModsSetupPage(ObjectSubclass<imp::AdventureModsSetupPage>)
        @extends gtk::Widget, adw::Bin,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

fn initial_preview_index(mod_count: usize, selected_mods: &[usize]) -> Option<usize> {
    selected_mods
        .iter()
        .copied()
        .find(|&idx| idx < mod_count)
        .or_else(|| (mod_count > 0).then_some(0))
}

fn subtitle_language_labels(game_kind: crate::steam::game::GameKind) -> Vec<&'static str> {
    config::SubtitleLanguage::supported_for(game_kind)
        .iter()
        .map(|language| language.label())
        .collect()
}

fn voice_language_labels() -> Vec<&'static str> {
    config::VoiceLanguage::all()
        .iter()
        .map(|language| language.label())
        .collect()
}

fn completed_mod_fraction(index: usize, total: usize) -> f64 {
    if total == 0 {
        return 0.0;
    }

    index.saturating_sub(1) as f64 / total as f64
}

fn mod_download_fraction(
    index: usize,
    total: usize,
    downloaded: u64,
    total_bytes: Option<u64>,
) -> f64 {
    let completed = completed_mod_fraction(index, total);
    let Some(total_bytes) = total_bytes else {
        return completed;
    };
    if total == 0 || total_bytes == 0 {
        return completed;
    }

    (completed + (downloaded as f64 / total_bytes as f64 / total as f64)).min(1.0)
}

struct ModDownloadProgressUpdate {
    fraction: f64,
    pulse: bool,
    text: String,
}

fn mod_download_progress_update(
    index: usize,
    total: usize,
    mod_name: &str,
    downloaded: u64,
    total_bytes: Option<u64>,
) -> ModDownloadProgressUpdate {
    let bytes_text = if let Some(tb) = total_bytes {
        format!(
            "{:.1} / {:.1} MB",
            downloaded as f64 / 1_048_576.0,
            tb as f64 / 1_048_576.0,
        )
    } else {
        format!("{:.1} MB", downloaded as f64 / 1_048_576.0)
    };

    ModDownloadProgressUpdate {
        fraction: mod_download_fraction(index, total, downloaded, total_bytes),
        pulse: total_bytes.is_none(),
        text: format!("{mod_name} ({index}/{total}) - {bytes_text}"),
    }
}

fn subtitle_language_index(
    game_kind: crate::steam::game::GameKind,
    language: config::SubtitleLanguage,
) -> u32 {
    config::SubtitleLanguage::supported_for(game_kind)
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0) as u32
}

fn voice_language_index(language: config::VoiceLanguage) -> u32 {
    config::VoiceLanguage::all()
        .iter()
        .position(|candidate| *candidate == language)
        .unwrap_or(0) as u32
}

fn populate_mod_preview(
    title_label: &gtk::Label,
    carousel: &adw::Carousel,
    carousel_frame: &gtk::Frame,
    description_label: &gtk::Label,
    links_box: &gtk::Box,
    mod_entry: Option<&common::ModEntry>,
) {
    let mut children = Vec::new();
    let mut child = carousel.first_child();
    while let Some(widget) = child {
        children.push(widget.clone());
        child = widget.next_sibling();
    }
    for child in children {
        carousel.remove(&child);
    }

    let (name, pictures, description, links) = if let Some(mod_entry) = mod_entry {
        (
            mod_entry.name,
            mod_entry.pictures,
            mod_entry.full_description.unwrap_or(mod_entry.description),
            mod_entry.links,
        )
    } else {
        ("", &[][..], "", &[][..])
    };

    title_label.set_label(name);

    if pictures.is_empty() {
        carousel_frame.set_visible(false);
    } else {
        carousel_frame.set_visible(true);
        for pic in pictures {
            let img = gtk::Picture::builder()
                .can_shrink(true)
                .content_fit(gtk::ContentFit::Contain)
                .hexpand(true)
                .vexpand(true)
                .build();
            img.set_resource(Some(*pic));

            let badge_text = if pic.contains("_before") {
                Some("Before")
            } else if pic.contains("_after") {
                Some("After")
            } else {
                None
            };

            if let Some(text) = badge_text {
                let badge = gtk::Label::builder()
                    .label(text)
                    .halign(gtk::Align::Center)
                    .valign(gtk::Align::End)
                    .margin_bottom(8)
                    .css_classes(vec!["caption".to_string(), "osd".to_string()])
                    .build();
                let overlay = gtk::Overlay::builder()
                    .child(&img)
                    .hexpand(true)
                    .vexpand(true)
                    .build();
                overlay.add_overlay(&badge);
                carousel.append(&overlay);
            } else {
                carousel.append(&img);
            }
        }
    }

    description_label.set_label(description);

    // Update links
    while let Some(child) = links_box.first_child() {
        links_box.remove(&child);
    }
    for link in links {
        let button = gtk::LinkButton::builder()
            .label(link.label)
            .uri(link.url)
            .halign(gtk::Align::Start)
            .build();
        links_box.append(&button);
    }
}

fn populate_mod_preview_for_index(
    title_label: &gtk::Label,
    carousel: &adw::Carousel,
    carousel_frame: &gtk::Frame,
    description_label: &gtk::Label,
    links_box: &gtk::Box,
    game_kind: crate::steam::game::GameKind,
    index: usize,
) {
    let mods = common::recommended_mods_for_game(game_kind);
    populate_mod_preview(
        title_label,
        carousel,
        carousel_frame,
        description_label,
        links_box,
        mods.get(index),
    );
}

impl AdventureModsSetupPage {
    pub fn new(game: Game) -> Self {
        let obj: Self = glib::Object::builder().build();
        let game_kind = game.kind;
        let all_steps = steps::steps_for_game(game.kind);
        obj.imp().all_steps.replace(all_steps);
        obj.imp().game.replace(Some(game));
        obj.imp()
            .language_selection
            .replace(Some(config::load_language_selection(
                config::app_settings().as_ref(),
                game_kind,
            )));

        let initial_step = obj.skip_completed_steps(0);
        obj.imp().current_step.set(initial_step);
        obj.show_current_step();
        obj
    }

    fn show_current_step(&self) {
        let imp = self.imp();
        let step_idx = imp.current_step.get();
        let all_steps = imp.all_steps.borrow();
        imp.is_error.set(false);

        // Cancel any in-flight operation
        if let Some(flag) = imp.cancel_flag.borrow().as_ref() {
            flag.store(true, Ordering::Relaxed);
        }
        imp.cancel_flag.replace(None);

        let Some(step) = all_steps.get(step_idx) else {
            return;
        };

        let centered_layout = !matches!(step.kind, steps::StepKind::ModSelection);
        imp.body_box.set_valign(if centered_layout {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        });
        imp.content_box.set_halign(if centered_layout {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        });
        imp.content_box.set_valign(if centered_layout {
            gtk::Align::Center
        } else {
            gtk::Align::Fill
        });
        imp.content_box.set_vexpand(!centered_layout);

        imp.step_title.set_label(step.title);
        let step_description = if step.id == "steam_config" {
            imp.game
                .borrow()
                .as_ref()
                .map(common::steam_config_message)
                .unwrap_or_else(|| step.description.to_string())
        } else {
            step.description.to_string()
        };
        imp.step_description.set_label(&step_description);

        let is_last_step = step_idx + 1 >= all_steps.len();
        imp.back_button.set_visible(!is_last_step);
        imp.back_button.set_sensitive(true);

        // Clear content box
        let content_box = &imp.content_box;
        while let Some(child) = content_box.first_child() {
            content_box.remove(&child);
        }

        match &step.kind {
            steps::StepKind::Auto => {
                imp.next_button.set_label("Continue");
                imp.next_button.set_sensitive(false);

                let spinner = gtk::Spinner::builder()
                    .spinning(true)
                    .halign(gtk::Align::Center)
                    .build();
                content_box.append(&spinner);

                self.run_auto_step(step.id);
            }
            steps::StepKind::Info => {
                imp.next_button
                    .set_label(if is_last_step { "Finish" } else { "Continue" });
                imp.next_button.set_sensitive(true);

                if step.id == "language_options" {
                    let selection = self.current_language_selection();
                    let game_kind = imp.game.borrow().as_ref().map(|game| game.kind);
                    let subtitle_languages = game_kind
                        .map(config::SubtitleLanguage::supported_for)
                        .unwrap_or(config::SubtitleLanguage::supported_for(
                            crate::steam::game::GameKind::SADX,
                        ));

                    let form_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Vertical)
                        .spacing(12)
                        .hexpand(true)
                        .build();

                    let subtitle_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(12)
                        .build();
                    let subtitle_dropdown = gtk::DropDown::from_strings(&subtitle_language_labels(
                        game_kind.unwrap_or(crate::steam::game::GameKind::SADX),
                    ));
                    subtitle_dropdown.set_selected(subtitle_language_index(
                        game_kind.unwrap_or(crate::steam::game::GameKind::SADX),
                        selection.subtitle,
                    ));
                    subtitle_box.append(
                        &gtk::Label::builder()
                            .label("Subtitles")
                            .halign(gtk::Align::Start)
                            .hexpand(true)
                            .build(),
                    );
                    subtitle_box.append(&subtitle_dropdown);

                    let voice_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(12)
                        .build();
                    let voice_dropdown = gtk::DropDown::from_strings(&voice_language_labels());
                    voice_dropdown.set_selected(voice_language_index(selection.voice));
                    voice_box.append(
                        &gtk::Label::builder()
                            .label("Voice Language")
                            .halign(gtk::Align::Start)
                            .hexpand(true)
                            .build(),
                    );
                    voice_box.append(&voice_dropdown);

                    let obj = self.clone();
                    let subtitle_languages = subtitle_languages.to_vec();
                    subtitle_dropdown.connect_selected_notify(move |dropdown| {
                        let language = subtitle_languages
                            .get(dropdown.selected() as usize)
                            .copied()
                            .unwrap_or(config::SubtitleLanguage::English);
                        let mut selection = obj.current_language_selection();
                        selection.subtitle = language;
                        obj.imp().language_selection.replace(Some(selection));
                    });

                    let obj = self.clone();
                    voice_dropdown.connect_selected_notify(move |dropdown| {
                        let language = config::VoiceLanguage::all()
                            .get(dropdown.selected() as usize)
                            .copied()
                            .unwrap_or(config::VoiceLanguage::English);
                        let mut selection = obj.current_language_selection();
                        selection.voice = language;
                        obj.imp().language_selection.replace(Some(selection));
                    });

                    form_box.append(&subtitle_box);
                    form_box.append(&voice_box);
                    content_box.append(&form_box);
                }
            }
            steps::StepKind::ExternalAction { button_label } => {
                imp.next_button.set_label("Continue");
                imp.next_button.set_sensitive(true);

                let action_button = gtk::Button::builder()
                    .label(*button_label)
                    .halign(gtk::Align::Center)
                    .css_classes(vec!["pill".to_string()])
                    .build();

                let step_id = step.id;
                let game = imp.game.borrow().clone();
                action_button.connect_clicked(move |btn| {
                    btn.set_sensitive(false);
                    if let Some(ref game) = game {
                        Self::run_external_action(step_id, game.clone(), btn.clone());
                    }
                });

                content_box.append(&action_button);
            }
            steps::StepKind::Download => {
                imp.next_button.set_label("Continue");
                imp.next_button.set_sensitive(false);

                let progress_bar = gtk::ProgressBar::builder()
                    .show_text(true)
                    .hexpand(true)
                    .build();

                let cancel_flag = Arc::new(AtomicBool::new(false));
                imp.cancel_flag.replace(Some(cancel_flag.clone()));

                content_box.append(&progress_bar);

                if step.id == "download_mods" {
                    let cancel_button = gtk::Button::builder()
                        .label("Cancel")
                        .halign(gtk::Align::Center)
                        .css_classes(vec!["destructive-action".to_string()])
                        .build();

                    let flag = cancel_flag.clone();
                    let obj = self.clone();
                    cancel_button.connect_clicked(move |btn| {
                        flag.store(true, Ordering::Relaxed);
                        btn.set_sensitive(false);
                        btn.set_label("Cancelling...");
                        // Re-show the step so user can retry
                        let obj2 = obj.clone();
                        glib::timeout_add_local_once(
                            std::time::Duration::from_millis(500),
                            move || {
                                obj2.show_current_step();
                            },
                        );
                    });

                    content_box.append(&cancel_button);
                }

                self.run_download_step(step.id, progress_bar, cancel_flag);
            }
            steps::StepKind::ModSelection => {
                imp.next_button.set_label("Install Selected");
                imp.next_button.set_sensitive(true);

                let game_kind = imp.game.borrow().as_ref().map(|g| g.kind);
                let presets = game_kind.map(common::presets_for_game).unwrap_or(&[]);

                let main_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .homogeneous(true)
                    .spacing(24)
                    .hexpand(true)
                    .vexpand(true)
                    .valign(gtk::Align::Fill)
                    .halign(gtk::Align::Fill)
                    .build();

                let left_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(12)
                    .hexpand(true)
                    .vexpand(true)
                    .valign(gtk::Align::Fill)
                    .build();

                let scrolled = gtk::ScrolledWindow::builder()
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .vscrollbar_policy(gtk::PolicyType::Automatic)
                    .hexpand(true)
                    .vexpand(true)
                    .valign(gtk::Align::Fill)
                    .build();

                let list_box = gtk::ListBox::builder()
                    .selection_mode(gtk::SelectionMode::Single)
                    .css_classes(vec!["boxed-list".to_string()])
                    .build();

                let checks: std::rc::Rc<std::cell::RefCell<Vec<gtk::CheckButton>>> =
                    std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));

                if !presets.is_empty() {
                    let preset_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(12)
                        .margin_bottom(6)
                        .build();

                    let preset_label = gtk::Label::builder()
                        .label("Preset:")
                        .css_classes(vec!["heading".to_string()])
                        .build();

                    let preset_names: Vec<&str> = presets.iter().map(|p| p.name).collect();
                    let dropdown = gtk::DropDown::from_strings(&preset_names);
                    dropdown.set_hexpand(true);

                    preset_box.append(&preset_label);
                    preset_box.append(&dropdown);
                    left_box.append(&preset_box);

                    let preset_desc_label = gtk::Label::builder()
                        .label(presets[0].description)
                        .wrap(true)
                        .halign(gtk::Align::Start)
                        .css_classes(vec!["caption".to_string()])
                        .margin_bottom(12)
                        .build();
                    left_box.append(&preset_desc_label);

                    let obj_clone = self.clone();
                    let presets_clone = presets;
                    let checks_clone = checks.clone();
                    let desc_label_clone = preset_desc_label.clone();
                    dropdown.connect_selected_notify(move |dd| {
                        let idx = dd.selected() as usize;
                        if let Some(preset) = presets_clone.get(idx) {
                            desc_label_clone.set_label(preset.description);

                            let mut sel = obj_clone.imp().selected_mods.borrow_mut();
                            sel.clear();

                            let game_kind = obj_clone.imp().game.borrow().as_ref().map(|g| g.kind);
                            let mods_list = game_kind
                                .map(common::recommended_mods_for_game)
                                .unwrap_or(&[]);

                            for (i, check) in checks_clone.borrow().iter().enumerate() {
                                if let Some(mod_entry) = mods_list.get(i) {
                                    let active = preset.mod_names.contains(&mod_entry.name);
                                    check.set_active(active);
                                    if active {
                                        sel.push(i);
                                    }
                                }
                            }
                        }
                    });
                }

                let preview_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(12)
                    .hexpand(true)
                    .vexpand(true)
                    .valign(gtk::Align::Fill)
                    .halign(gtk::Align::Fill)
                    .build();

                let carousel = adw::Carousel::builder()
                    .interactive(true)
                    .allow_scroll_wheel(true)
                    .vexpand(true)
                    .build();

                let indicator = adw::CarouselIndicatorDots::builder()
                    .carousel(&carousel)
                    .margin_top(6)
                    .margin_bottom(6)
                    .build();

                let carousel_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .build();

                carousel_box.append(&carousel);
                carousel_box.append(&indicator);

                let carousel_frame = gtk::Frame::builder()
                    .child(&carousel_box)
                    .height_request(MOD_PREVIEW_IMAGE_HEIGHT)
                    .hexpand(true)
                    .vexpand(true)
                    .build();

                let full_desc_label = gtk::Label::builder()
                    .wrap(true)
                    .halign(gtk::Align::Start)
                    .valign(gtk::Align::Start)
                    .css_classes(vec!["body".to_string()])
                    .build();

                let desc_scrolled = gtk::ScrolledWindow::builder()
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .vscrollbar_policy(gtk::PolicyType::Automatic)
                    .max_content_height(MOD_PREVIEW_DESCRIPTION_HEIGHT)
                    .hexpand(true)
                    .vexpand(true)
                    .child(&full_desc_label)
                    .build();

                let preview_title_label = gtk::Label::builder()
                    .halign(gtk::Align::Start)
                    .css_classes(vec!["title-3".to_string()])
                    .build();

                let links_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(6)
                    .halign(gtk::Align::Start)
                    .build();

                preview_box.append(&preview_title_label);
                preview_box.append(&carousel_frame);
                preview_box.append(&desc_scrolled);
                preview_box.append(&links_box);

                let mods_list = game_kind
                    .map(common::recommended_mods_for_game)
                    .unwrap_or(&[]);
                let preview_game_kind = game_kind.unwrap_or(crate::steam::game::GameKind::SADX);
                let mut initial_selected = Vec::new();

                {
                    let preview_title_clone = preview_title_label.clone();
                    let carousel_clone = carousel.clone();
                    let carousel_frame_clone = carousel_frame.clone();
                    let desc_lbl_clone = full_desc_label.clone();
                    let links_box_clone = links_box.clone();
                    list_box.connect_row_selected(move |_, row| {
                        let Some(row) = row else { return };
                        populate_mod_preview_for_index(
                            &preview_title_clone,
                            &carousel_clone,
                            &carousel_frame_clone,
                            &desc_lbl_clone,
                            &links_box_clone,
                            preview_game_kind,
                            row.index() as usize,
                        );
                    });
                }

                let default_preset = presets.first();

                for (i, mod_entry) in mods_list.iter().enumerate() {
                    let row_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(12)
                        .margin_start(12)
                        .margin_end(12)
                        .margin_top(12)
                        .margin_bottom(12)
                        .hexpand(true)
                        .build();

                    let is_active = default_preset
                        .map(|preset| preset.mod_names.contains(&mod_entry.name))
                        .unwrap_or(true);

                    let check = gtk::CheckButton::builder().active(is_active).build();
                    checks.borrow_mut().push(check.clone());

                    let text_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Vertical)
                        .spacing(2)
                        .hexpand(true)
                        .build();

                    let name_label = gtk::Label::builder()
                        .label(mod_entry.name)
                        .halign(gtk::Align::Start)
                        .css_classes(vec!["heading".to_string()])
                        .build();

                    let desc_label = gtk::Label::builder()
                        .label(mod_entry.description)
                        .halign(gtk::Align::Start)
                        .wrap(true)
                        .css_classes(vec!["caption".to_string()])
                        .build();

                    text_box.append(&name_label);
                    text_box.append(&desc_label);

                    row_box.append(&check);
                    row_box.append(&text_box);

                    let list_row = gtk::ListBoxRow::builder().child(&row_box).build();

                    let obj_clone = self.clone();
                    let idx = i;
                    check.connect_toggled(move |btn| {
                        if let Ok(mut sel) = obj_clone.imp().selected_mods.try_borrow_mut() {
                            if btn.is_active() {
                                if !sel.contains(&idx) {
                                    sel.push(idx);
                                }
                            } else {
                                sel.retain(|&x| x != idx);
                            }
                        }
                    });

                    let preview_title_clone = preview_title_label.clone();
                    let carousel_clone = carousel.clone();
                    let carousel_frame_clone = carousel_frame.clone();
                    let desc_lbl_clone = full_desc_label.clone();
                    let links_box_clone = links_box.clone();
                    check.connect_has_focus_notify(move |btn| {
                        if btn.has_focus() {
                            populate_mod_preview_for_index(
                                &preview_title_clone,
                                &carousel_clone,
                                &carousel_frame_clone,
                                &desc_lbl_clone,
                                &links_box_clone,
                                preview_game_kind,
                                idx,
                            );
                        }
                    });

                    // Preview update on row focus/motion
                    let preview_title_clone = preview_title_label.clone();
                    let carousel_clone = carousel.clone();
                    let carousel_frame_clone = carousel_frame.clone();
                    let desc_lbl_clone = full_desc_label.clone();
                    let links_box_clone = links_box.clone();
                    let mod_entry_clone = mod_entry;

                    let gesture = gtk::EventControllerMotion::new();
                    gesture.connect_enter(move |_, _, _| {
                        populate_mod_preview(
                            &preview_title_clone,
                            &carousel_clone,
                            &carousel_frame_clone,
                            &desc_lbl_clone,
                            &links_box_clone,
                            Some(mod_entry_clone),
                        );
                    });
                    list_row.add_controller(gesture);

                    list_box.append(&list_row);
                    if is_active {
                        initial_selected.push(i);
                    }
                }
                imp.selected_mods.replace(initial_selected);

                let preview_entry =
                    initial_preview_index(mods_list.len(), &imp.selected_mods.borrow())
                        .and_then(|idx| mods_list.get(idx));
                populate_mod_preview(
                    &preview_title_label,
                    &carousel,
                    &carousel_frame,
                    &full_desc_label,
                    &links_box,
                    preview_entry,
                );
                if let Some(initial_index) =
                    initial_preview_index(mods_list.len(), &imp.selected_mods.borrow())
                    && let Some(row) = list_box.row_at_index(initial_index as i32)
                {
                    list_box.select_row(Some(&row));
                }

                scrolled.set_child(Some(&list_box));
                left_box.append(&scrolled);
                main_box.append(&left_box);
                main_box.append(&preview_box);
                content_box.append(&main_box);
            }
        }
    }

    fn run_auto_step(&self, step_id: &'static str) {
        let obj = self.clone();
        let game = self.imp().game.borrow().clone();

        glib::spawn_future_local(async move {
            let result = match step_id {
                "dotnet" => {
                    if let Some(ref game) = game {
                        common::install_runtimes(game.path.clone(), game.kind.app_id()).await
                    } else {
                        Ok(())
                    }
                }
                _ => Ok(()),
            };

            match result {
                Ok(()) => {
                    obj.imp().next_button.set_sensitive(true);
                    obj.advance_step();
                }
                Err(e) => {
                    obj.show_error(&format!("{e}"));
                }
            }
        });
    }

    fn run_external_action(step_id: &'static str, game: Game, button: gtk::Button) {
        if step_id == "steam_config"
            && let Some(setup_page) = button
                .ancestor(AdventureModsSetupPage::static_type())
                .and_then(|w| w.downcast::<AdventureModsSetupPage>().ok())
        {
            let msg = common::steam_config_message(&game);
            setup_page.imp().step_description.set_label(&msg);
        }
        button.set_sensitive(true);
    }

    fn run_download_step(
        &self,
        step_id: &'static str,
        progress_bar: gtk::ProgressBar,
        cancel_flag: Arc<AtomicBool>,
    ) {
        let obj = self.clone();
        let game = self.imp().game.borrow().clone();

        glib::spawn_future_local(async move {
            let Some(ref game) = game else { return };

            // Channel messages for progress bar updates
            enum ProgressMsg {
                // Byte-level download progress (for single-file steps)
                Bytes {
                    downloaded: u64,
                    total: Option<u64>,
                    status: String,
                },
                // Mod install: fraction by item index, label shows mod name
                ModInstall {
                    index: usize,
                    total: usize,
                    mod_name: String,
                },
                // Per-mod byte download progress (nested inside mod install)
                ModBytes {
                    index: usize,
                    total: usize,
                    mod_name: String,
                    downloaded: u64,
                    total_bytes: Option<u64>,
                },
                Configuring {
                    total: usize,
                },
            }

            let (tx, rx) = async_channel::bounded::<ProgressMsg>(32);

            // Progress update receiver
            let pb = progress_bar.clone();
            glib::spawn_future_local(async move {
                while let Ok(msg) = rx.recv().await {
                    match msg {
                        ProgressMsg::Bytes {
                            downloaded,
                            total,
                            status,
                        } => {
                            let bytes_text = if let Some(total) = total {
                                if total > 0 {
                                    pb.set_fraction(downloaded as f64 / total as f64);
                                }
                                format!(
                                    "{:.1} / {:.1} MB",
                                    downloaded as f64 / 1_048_576.0,
                                    total as f64 / 1_048_576.0,
                                )
                            } else {
                                pb.pulse();
                                format!("{:.1} MB", downloaded as f64 / 1_048_576.0)
                            };
                            let text = if status.is_empty() {
                                bytes_text
                            } else {
                                format!("{status} ({bytes_text})")
                            };
                            pb.set_text(Some(&text));
                        }
                        ProgressMsg::ModInstall {
                            index,
                            total,
                            mod_name,
                        } => {
                            pb.set_fraction(completed_mod_fraction(index, total));
                            pb.set_text(Some(&format!("{mod_name} ({index}/{total})")));
                        }
                        ProgressMsg::ModBytes {
                            index,
                            total,
                            mod_name,
                            downloaded,
                            total_bytes,
                        } => {
                            let update = mod_download_progress_update(
                                index,
                                total,
                                &mod_name,
                                downloaded,
                                total_bytes,
                            );
                            if update.pulse {
                                pb.pulse();
                            } else {
                                pb.set_fraction(update.fraction);
                            }
                            pb.set_text(Some(&update.text));
                        }
                        ProgressMsg::Configuring { total } => {
                            pb.set_fraction(1.0);
                            pb.set_text(Some(&format!("Configuring... ({total}/{total})")));
                        }
                    }
                }
            });

            let game_kind = game.kind;
            let (width, height) = obj.get_resolution();
            let result: anyhow::Result<()> = match step_id {
                "convert_steam" => {
                    let game_path = game.path.clone();
                    let tx_clone = tx.clone();
                    let progress_fn: Option<crate::external::download::ProgressFn> =
                        Some(Box::new(move |dl, total| {
                            let _ = tx_clone.send_blocking(ProgressMsg::Bytes {
                                downloaded: dl,
                                total,
                                status: "Downloading...".to_string(),
                            });
                        }));
                    blocking::flatten_spawn_result(
                        gio::spawn_blocking(move || {
                            sadx::convert_steam_to_2004(&game_path, progress_fn)
                        })
                        .await,
                    )
                }
                "install_mod_manager" => {
                    let game_path = game.path.clone();
                    let game_kind = game.kind;
                    let tx_clone = tx.clone();
                    let progress_fn: Option<crate::external::download::ProgressFn> =
                        Some(Box::new(move |dl, total| {
                            let _ = tx_clone.send_blocking(ProgressMsg::Bytes {
                                downloaded: dl,
                                total,
                                status: "Downloading...".to_string(),
                            });
                        }));
                    blocking::flatten_spawn_result(
                        gio::spawn_blocking(move || {
                            common::install_mod_manager(&game_path, game_kind, progress_fn)
                        })
                        .await,
                    )
                }
                "download_mods" => {
                    let selected: Vec<usize> = obj.imp().selected_mods.borrow().clone();
                    let total_count = selected.len();
                    let game_path = game.path.clone();
                    let language_selection = obj.current_language_selection();
                    let was_cancelled = cancel_flag.clone();
                    let cancel_during_install = cancel_flag.clone();
                    let mods_list = common::recommended_mods_for_game(game_kind);
                    match blocking::flatten_spawn_result(
                        gio::spawn_blocking(move || {
                            let selected_entries: Vec<&common::ModEntry> = selected
                                .iter()
                                .filter_map(|idx| mods_list.get(*idx))
                                .collect();
                            pipeline::install_selected_mods_and_generate_config_with_progress(
                                &game_path,
                                game_kind,
                                &selected_entries,
                                width,
                                height,
                                language_selection,
                                |progress| {
                                    if cancel_during_install.load(Ordering::Relaxed) {
                                        return Err(anyhow::anyhow!("cancelled"));
                                    }
                                    match progress {
                                        pipeline::InstallProgress::InstallingMod {
                                            index,
                                            total,
                                            mod_name,
                                        } => {
                                            let _ = tx.send_blocking(ProgressMsg::ModInstall {
                                                index,
                                                total,
                                                mod_name: mod_name.to_string(),
                                            });
                                        }
                                        pipeline::InstallProgress::DownloadingMod {
                                            index,
                                            total,
                                            mod_name,
                                            downloaded,
                                            total_bytes,
                                        } => {
                                            let _ = tx.send_blocking(ProgressMsg::ModBytes {
                                                index,
                                                total,
                                                mod_name: mod_name.to_string(),
                                                downloaded,
                                                total_bytes,
                                            });
                                        }
                                        pipeline::InstallProgress::GeneratingConfig => {
                                            let _ = tx.send_blocking(ProgressMsg::Configuring {
                                                total: total_count,
                                            });
                                        }
                                    }
                                    Ok(())
                                },
                            )
                        })
                        .await,
                    ) {
                        Ok(()) => Ok(()),
                        Err(e) => {
                            if was_cancelled.load(Ordering::Relaxed) {
                                return; // Was cancelled
                            }
                            Err(e)
                        }
                    }
                }
                _ => Ok(()),
            };

            match result {
                Ok(()) => {
                    obj.imp().next_button.set_sensitive(true);
                    obj.advance_step();
                }
                Err(e) => {
                    obj.show_error(&format!("{e}"));
                }
            }
        });
    }

    fn get_resolution(&self) -> (u32, u32) {
        // Fallback to 1080p if we can't detect it
        let default_res = (1920, 1080);

        let display = self.display();
        let monitors = display.monitors();

        // Try to get the monitor where the window is
        let surface = self.native().and_then(|n| n.surface());
        let monitor = if let Some(ref s) = surface {
            display.monitor_at_surface(s)
        } else {
            monitors
                .item(0)
                .and_then(|m| m.downcast::<gdk::Monitor>().ok())
        };

        if let Some(monitor) = monitor {
            let geometry = monitor.geometry();

            // Try to get fractional scale from surface, fall back to monitor's integer scale.
            // gdk::Surface::scale() returns f64 and supports fractional scaling (GTK 4.12+).
            let scale = if let Some(s) = surface {
                s.scale()
            } else {
                monitor.scale_factor() as f64
            };

            let (width, height) = (
                (geometry.width() as f64 * scale).round() as u32,
                (geometry.height() as f64 * scale).round() as u32,
            );
            tracing::info!(
                "Detected resolution: {}x{} (Logical: {}x{}, Scale: {:.2})",
                width,
                height,
                geometry.width(),
                geometry.height(),
                scale
            );
            (width, height)
        } else {
            tracing::warn!(
                "Could not detect monitor resolution, using fallback {}x{}",
                default_res.0,
                default_res.1
            );
            default_res
        }
    }
    fn advance_step(&self) {
        let imp = self.imp();
        let next = imp.current_step.get() + 1;
        let total = imp.all_steps.borrow().len();

        if next < total {
            let next_skipped = self.skip_completed_steps(next);
            imp.current_step.set(next_skipped);
            self.show_current_step();
        }
    }

    fn skip_completed_steps(&self, start_idx: usize) -> usize {
        let imp = self.imp();
        let all_steps = imp.all_steps.borrow();
        let game = imp.game.borrow().clone().unwrap();
        let mut idx = start_idx;

        // Skip steps that are already complete, but NEVER skip the last step
        // (the completion screen).
        while idx < all_steps.len() - 1 {
            let step = &all_steps[idx];
            if common::is_step_complete(step.id, &game) {
                tracing::info!("Auto-skipping completed step: {}", step.id);
                idx += 1;
            } else {
                break;
            }
        }
        idx
    }

    fn on_next_clicked(&self) {
        if self.imp().is_error.get() {
            // Retry: re-run the current step
            self.show_current_step();
        } else {
            let imp = self.imp();
            let current_step_id = imp.all_steps.borrow()[imp.current_step.get()].id;
            let next = imp.current_step.get() + 1;
            let total = imp.all_steps.borrow().len();
            if next >= total {
                // Last step: navigate back to the welcome page
                self.go_back_to_welcome();
            } else {
                if current_step_id == "language_options" {
                    self.persist_language_selection();
                }
                self.advance_step();
            }
        }
    }

    fn current_language_selection(&self) -> config::LanguageSelection {
        self.imp().language_selection.borrow().unwrap_or_else(|| {
            self.imp()
                .game
                .borrow()
                .as_ref()
                .map(|game| config::LanguageSelection::defaults_for(game.kind))
                .unwrap_or(config::LanguageSelection::defaults_for(
                    crate::steam::game::GameKind::SADX,
                ))
        })
    }

    fn persist_language_selection(&self) {
        let Some(game) = self.imp().game.borrow().clone() else {
            return;
        };

        config::save_language_selection(
            config::app_settings().as_ref(),
            game.kind,
            self.current_language_selection(),
        );
    }

    fn on_back_clicked(&self) {
        let imp = self.imp();
        let current = imp.current_step.get();
        if current == 0 {
            self.go_back_to_welcome();
            return;
        }

        let all_steps = imp.all_steps.borrow();
        let game = imp.game.borrow().clone().unwrap();
        let mut prev = current - 1;

        // Skip steps backwards that are either automatic or already complete
        while prev > 0 {
            let step = &all_steps[prev];
            if matches!(step.kind, steps::StepKind::Auto)
                || common::is_step_complete(step.id, &game)
            {
                prev -= 1;
            } else {
                break;
            }
        }

        // Final check for the step we landed on: if it's still something that should be skipped,
        // it means we've reached the beginning of the list and everything before 'current' was skippable.
        let step = &all_steps[prev];
        if matches!(step.kind, steps::StepKind::Auto) || common::is_step_complete(step.id, &game) {
            self.go_back_to_welcome();
        } else {
            imp.current_step.set(prev);
            self.show_current_step();
        }
    }

    fn go_back_to_welcome(&self) {
        if let Some(nav_view) = self.ancestor(adw::NavigationView::static_type()) {
            let nav_view: adw::NavigationView = nav_view.downcast().unwrap();
            nav_view.pop();
        }
    }

    fn show_error(&self, message: &str) {
        let imp = self.imp();
        imp.is_error.set(true);

        let content_box = &imp.content_box;
        while let Some(child) = content_box.first_child() {
            content_box.remove(&child);
        }

        let label = gtk::Label::builder()
            .label(message)
            .wrap(true)
            .css_classes(vec!["error".to_string()])
            .build();
        content_box.append(&label);

        imp.next_button.set_label("Retry");
        imp.next_button.set_sensitive(true);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use adw::prelude::*;
    use adw::subclass::prelude::ObjectSubclassIsExt;
    use gtk::glib;

    use super::AdventureModsSetupPage;
    use super::{
        completed_mod_fraction, initial_preview_index, mod_download_fraction,
        mod_download_progress_update, subtitle_language_labels, voice_language_labels,
    };
    use crate::steam::game::Game;
    use crate::steam::game::GameKind;

    fn init_resource_overlay() {
        static INIT: Once = Once::new();

        INIT.call_once(|| unsafe {
            std::env::set_var(
                "G_RESOURCE_OVERLAYS",
                concat!(
                    "/io/github/astrovm/AdventureMods=",
                    env!("CARGO_MANIFEST_DIR"),
                    "/data"
                ),
            );
        });
    }

    #[test]
    fn initial_preview_prefers_first_selected_mod() {
        assert_eq!(initial_preview_index(5, &[3, 1]), Some(3));
    }

    #[test]
    fn initial_preview_falls_back_to_first_mod_when_none_selected() {
        assert_eq!(initial_preview_index(5, &[]), Some(0));
    }

    #[test]
    fn initial_preview_skips_out_of_range_selection() {
        assert_eq!(initial_preview_index(2, &[4, 1]), Some(1));
    }

    #[test]
    fn initial_preview_is_none_when_no_mods_exist() {
        assert_eq!(initial_preview_index(0, &[0]), None);
    }

    #[test]
    fn initial_preview_is_none_when_no_mods_and_no_selection() {
        assert_eq!(initial_preview_index(0, &[]), None);
    }

    #[test]
    fn initial_preview_falls_back_to_zero_when_all_selections_out_of_range() {
        // All selected indices are out of range, but mods exist
        assert_eq!(initial_preview_index(3, &[5, 10, 99]), Some(0));
    }

    #[test]
    fn sadx_subtitle_options_include_expected_values() {
        assert_eq!(
            subtitle_language_labels(GameKind::SADX),
            vec!["日本語", "English", "Français", "Español", "Deutsch"]
        );
    }

    #[test]
    fn sa2_subtitle_options_include_expected_values() {
        assert_eq!(
            subtitle_language_labels(GameKind::SA2),
            vec![
                "English",
                "Deutsch",
                "Español",
                "Français",
                "Italiano",
                "日本語"
            ]
        );
    }

    #[test]
    fn voice_options_include_expected_values() {
        assert_eq!(voice_language_labels(), vec!["日本語", "English"]);
    }

    #[test]
    fn mod_download_fraction_starts_at_prior_completed_items() {
        assert_eq!(mod_download_fraction(1, 2, 0, Some(100)), 0.0);
        assert_eq!(mod_download_fraction(2, 2, 0, Some(100)), 0.5);
    }

    #[test]
    fn mod_download_fraction_reaches_completion_at_end_of_last_item() {
        assert_eq!(mod_download_fraction(2, 2, 100, Some(100)), 1.0);
    }

    #[test]
    fn mod_download_progress_pulses_when_total_size_is_unknown() {
        let update = mod_download_progress_update(2, 4, "Render Fix", 1_048_576, None);

        assert_eq!(update.fraction, completed_mod_fraction(2, 4));
        assert!(update.pulse);
        assert_eq!(update.text, "Render Fix (2/4) - 1.0 MB");
    }

    #[gtk::test]
    fn selecting_mod_row_updates_preview_title() {
        init_resource_overlay();

        let tmp = tempfile::tempdir().unwrap();
        let page = AdventureModsSetupPage::new(Game {
            kind: GameKind::SA2,
            path: tmp.path().to_path_buf(),
        });
        let select_mods_index = page
            .imp()
            .all_steps
            .borrow()
            .iter()
            .position(|step| step.id == "select_mods")
            .unwrap();

        page.imp().current_step.set(select_mods_index);
        page.show_current_step();

        let main_box = page
            .imp()
            .content_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let left_box = main_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let scrolled = left_box
            .first_child()
            .unwrap()
            .downcast::<gtk::ScrolledWindow>()
            .unwrap();
        let viewport = scrolled
            .child()
            .unwrap()
            .downcast::<gtk::Viewport>()
            .unwrap();
        let list_box = viewport
            .child()
            .unwrap()
            .downcast::<gtk::ListBox>()
            .unwrap();
        let preview_box = main_box
            .last_child()
            .unwrap()
            .downcast::<gtk::Box>()
            .unwrap();
        let preview_title = preview_box
            .first_child()
            .unwrap()
            .downcast::<gtk::Label>()
            .unwrap();

        let row = list_box.row_at_index(1).unwrap();
        list_box.select_row(Some(&row));
        while glib::MainContext::default().iteration(false) {}

        assert_eq!(
            preview_title.label().as_str(),
            "Retranslated Story -COMPLETE-"
        );
    }
}
