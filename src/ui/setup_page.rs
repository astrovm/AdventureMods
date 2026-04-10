use std::cell::Cell;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gdk, gio, glib};

use crate::blocking;
use crate::setup::{common, pipeline, sadx, steps};
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

impl AdventureModsSetupPage {
    pub fn new(game: Game) -> Self {
        let obj: Self = glib::Object::builder().build();
        let all_steps = steps::steps_for_game(game.kind);
        obj.imp().all_steps.replace(all_steps);
        obj.imp().game.replace(Some(game));

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
                    .selection_mode(gtk::SelectionMode::None)
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
                let mut initial_selected = Vec::new();

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

            // Channel now carries (downloaded, total, status_text)
            let (tx, rx) = async_channel::bounded::<(u64, Option<u64>, String)>(32);

            // Progress update receiver
            let pb = progress_bar.clone();
            glib::spawn_future_local(async move {
                while let Ok((downloaded, total, status)) = rx.recv().await {
                    let mut text = if status.is_empty() {
                        String::new()
                    } else {
                        format!("{} ", status)
                    };

                    if let Some(total) = total {
                        if total > 0 {
                            let frac = downloaded as f64 / total as f64;
                            pb.set_fraction(frac);
                        }

                        // Large total means bytes, small means item count
                        if total > 1000 {
                            text.push_str(&format!(
                                "({:.1} / {:.1} MB)",
                                downloaded as f64 / 1_048_576.0,
                                total as f64 / 1_048_576.0,
                            ));
                        }
                    } else {
                        pb.pulse();
                        text.push_str(&format!("({:.1} MB)", downloaded as f64 / 1_048_576.0,));
                    }

                    let display_text = text.trim();
                    if display_text.is_empty() {
                        pb.set_text(None);
                    } else {
                        pb.set_text(Some(display_text));
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
                            let _ = tx_clone.send_blocking((dl, total, String::new()));
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
                            let _ = tx_clone.send_blocking((dl, total, String::new()));
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
                                            let status = format!("{mod_name} ({index}/{total})");
                                            let _ = tx.send_blocking((
                                                index as u64,
                                                Some(total as u64),
                                                status,
                                            ));
                                        }
                                        pipeline::InstallProgress::GeneratingConfig => {
                                            let _ = tx.send_blocking((
                                                total_count as u64,
                                                Some(total_count as u64),
                                                "Configuring...".to_string(),
                                            ));
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
            let next = imp.current_step.get() + 1;
            let total = imp.all_steps.borrow().len();
            if next >= total {
                // Last step: navigate back to the welcome page
                self.go_back_to_welcome();
            } else {
                self.advance_step();
            }
        }
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
    use super::initial_preview_index;

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
}
