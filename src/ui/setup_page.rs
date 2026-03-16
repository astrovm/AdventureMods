use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::{gio, glib};

use crate::setup::{common, sadx, steps};
use crate::steam::game::Game;

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/setup_page.ui")]
    pub struct AdventureModsSetupPage {
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

        imp.step_title.set_label(step.title);
        imp.step_description.set_label(step.description);
        imp.back_button.set_sensitive(step_idx > 0);

        // Clear content box
        let content_box = &imp.content_box;
        while let Some(child) = content_box.first_child() {
            content_box.remove(&child);
        }

        let is_last_step = step_idx + 1 >= all_steps.len();

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
                imp.next_button.set_label(if is_last_step { "Finish" } else { "Continue" });
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
                        Self::run_external_action(
                            step_id,
                            game.clone(),
                            btn.clone(),
                        );
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
                    glib::timeout_add_local_once(std::time::Duration::from_millis(500), move || {
                        obj2.show_current_step();
                    });
                });

                content_box.append(&progress_bar);
                content_box.append(&cancel_button);

                self.run_download_step(step.id, progress_bar, cancel_flag);
            }
            steps::StepKind::ModSelection => {
                imp.next_button.set_label("Install Selected");
                imp.next_button.set_sensitive(true);

                let main_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Horizontal)
                    .spacing(24)
                    .hexpand(true)
                    .build();

                let scrolled = gtk::ScrolledWindow::builder()
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .vscrollbar_policy(gtk::PolicyType::Automatic)
                    .max_content_height(400)
                    .propagate_natural_height(true)
                    .min_content_width(350)
                    .hexpand(true)
                    .build();

                let list_box = gtk::ListBox::builder()
                    .selection_mode(gtk::SelectionMode::None)
                    .css_classes(vec!["boxed-list".to_string()])
                    .build();

                let preview_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(12)
                    .valign(gtk::Align::Start)
                    .width_request(400)
                    .build();

                let before_label = gtk::Label::builder()
                    .label("Before")
                    .halign(gtk::Align::Start)
                    .css_classes(vec!["caption".to_string()])
                    .build();
                let before_image = gtk::Picture::builder()
                    .can_shrink(true)
                    .content_fit(gtk::ContentFit::Cover)
                    .height_request(200)
                    .build();

                let after_label = gtk::Label::builder()
                    .label("After")
                    .halign(gtk::Align::Start)
                    .css_classes(vec!["caption".to_string()])
                    .build();
                let after_image = gtk::Picture::builder()
                    .can_shrink(true)
                    .content_fit(gtk::ContentFit::Cover)
                    .height_request(200)
                    .build();

                preview_box.append(&before_label);
                preview_box.append(&before_image);
                preview_box.append(&after_label);
                preview_box.append(&after_image);

                preview_box.set_opacity(0.0);

                let game_kind = imp.game.borrow().as_ref().map(|g| g.kind);
                let mods_list = game_kind
                    .map(|k| common::recommended_mods_for_game(k))
                    .unwrap_or(&[]);
                let mut selected = Vec::new();

                for (i, mod_entry) in mods_list.iter().enumerate() {
                    let row_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Horizontal)
                        .spacing(12)
                        .margin_start(12)
                        .margin_end(12)
                        .margin_top(12)
                        .margin_bottom(12)
                        .build();

                    let check = gtk::CheckButton::builder()
                        .active(true)
                        .build();

                    let text_box = gtk::Box::builder()
                        .orientation(gtk::Orientation::Vertical)
                        .spacing(2)
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

                    let list_row = gtk::ListBoxRow::builder()
                        .child(&row_box)
                        .build();

                    let obj_clone = self.clone();
                    let idx = i;
                    check.connect_toggled(move |btn| {
                        let mut sel = obj_clone.imp().selected_mods.borrow_mut();
                        if btn.is_active() {
                            if !sel.contains(&idx) {
                                sel.push(idx);
                            }
                        } else {
                            sel.retain(|&x| x != idx);
                        }
                    });

                    // Preview update on row focus/selection
                    let b_img = before_image.clone();
                    let a_img = after_image.clone();
                    let p_box = preview_box.clone();
                    let mod_entry_clone = mod_entry;
                    
                    let gesture = gtk::EventControllerMotion::new();
                    gesture.connect_enter(move |_, _, _| {
                        if let Some(before) = mod_entry_clone.before_image {
                            b_img.set_resource(Some(before));
                            p_box.set_opacity(1.0);
                        } else {
                            p_box.set_opacity(0.0);
                        }
                        if let Some(after) = mod_entry_clone.after_image {
                            a_img.set_resource(Some(after));
                        }
                    });
                    list_row.add_controller(gesture);

                    list_box.append(&list_row);
                    selected.push(i);
                }
                imp.selected_mods.replace(selected);

                scrolled.set_child(Some(&list_box));
                main_box.append(&scrolled);
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
                "check_deps" => common::ensure_protontricks().await,
                "dotnet" => {
                    if let Some(ref game) = game {
                        common::install_dotnet(game.kind.app_id()).await
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

    fn run_external_action(
        step_id: &'static str,
        _game: Game,
        button: gtk::Button,
    ) {
        glib::spawn_future_local(async move {
            let result = match step_id {
                "ge_proton" => {
                    if let Err(e) = common::ensure_protonup().await {
                        Err(e)
                    } else {
                        common::launch_protonup().await
                    }
                }
                _ => Ok(()),
            };

            button.set_sensitive(true);
            if let Err(e) = result {
                tracing::error!("External action '{step_id}' failed: {e}");
            }
        });
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

            let (tx, rx) = async_channel::bounded::<(u64, Option<u64>)>(32);

            // Progress update receiver
            let pb = progress_bar.clone();
            glib::spawn_future_local(async move {
                while let Ok((downloaded, total)) = rx.recv().await {
                    if let Some(total) = total {
                        let frac = downloaded as f64 / total as f64;
                        pb.set_fraction(frac);
                        pb.set_text(Some(&format!(
                            "{:.1} / {:.1} MB",
                            downloaded as f64 / 1_048_576.0,
                            total as f64 / 1_048_576.0,
                        )));
                    } else {
                        pb.pulse();
                        pb.set_text(Some(&format!(
                            "{:.1} MB",
                            downloaded as f64 / 1_048_576.0,
                        )));
                    }
                }
            });

            let game_kind = game.kind;
            let result: anyhow::Result<()> = match step_id {
                "convert_steam" => {
                    let game_path = game.path.clone();
                    let progress_fn: Option<crate::external::download::ProgressFn> =
                        Some(Box::new(move |dl, total| {
                            let _ = tx.send_blocking((dl, total));
                        }));
                    match gio::spawn_blocking(move || {
                        sadx::convert_steam_to_2004(&game_path, progress_fn)
                    })
                    .await
                    {
                        Ok(inner) => inner,
                        Err(e) => Err(anyhow::anyhow!("spawn error: {e:?}")),
                    }
                }
                "install_mod_loader" => {
                    let game_path = game.path.clone();
                    let progress_fn: Option<crate::external::download::ProgressFn> =
                        Some(Box::new(move |dl, total| {
                            let _ = tx.send_blocking((dl, total));
                        }));
                    match gio::spawn_blocking(move || {
                        sadx::install_mod_loader(&game_path, progress_fn)
                    })
                    .await
                    {
                        Ok(inner) => inner,
                        Err(e) => Err(anyhow::anyhow!("spawn error: {e:?}")),
                    }
                }
                "install_mod_manager" => {
                    let game_path = game.path.clone();
                    let progress_fn: Option<crate::external::download::ProgressFn> =
                        Some(Box::new(move |dl, total| {
                            let _ = tx.send_blocking((dl, total));
                        }));
                    match gio::spawn_blocking(move || {
                        common::install_mod_manager(&game_path, progress_fn)
                    })
                    .await
                    {
                        Ok(inner) => inner,
                        Err(e) => Err(anyhow::anyhow!("spawn error: {e:?}")),
                    }
                }
                "download_mods" => {
                    let selected: Vec<usize> = obj.imp().selected_mods.borrow().clone();
                    let total = selected.len();
                    let game_path = game.path.clone();
                    let was_cancelled = cancel_flag.clone();
                    let mods_list = common::recommended_mods_for_game(game_kind);
                    match gio::spawn_blocking(move || {
                        let mut selected_entries = Vec::new();
                        for (i, idx) in selected.iter().enumerate() {
                            if cancel_flag.load(Ordering::Relaxed) {
                                return Err(anyhow::anyhow!("cancelled"));
                            }
                            if let Some(mod_entry) = mods_list.get(*idx) {
                                let _ = tx.send_blocking((i as u64, Some(total as u64)));
                                common::install_mod(&game_path, mod_entry, None)?;
                                selected_entries.push(mod_entry);
                            }
                        }

                        // Configure SADX mod loader if applicable
                        if game_kind == crate::steam::game::GameKind::SADX {
                            sadx::configure_mod_loader(&game_path, &selected_entries)?;
                        }

                        Ok(())
                    })
                    .await
                    {
                        Ok(Ok(())) => Ok(()),
                        Ok(Err(e)) => {
                            if was_cancelled.load(Ordering::Relaxed) {
                                return; // Was cancelled
                            }
                            Err(e)
                        }
                        Err(e) => Err(anyhow::anyhow!("spawn error: {e:?}")),
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
                // Last step — navigate back to the welcome page
                if let Some(nav_view) = self.ancestor(adw::NavigationView::static_type()) {
                    let nav_view: adw::NavigationView = nav_view.downcast().unwrap();
                    nav_view.pop();
                }
            } else {
                self.advance_step();
            }
        }
    }

    fn on_back_clicked(&self) {
        let imp = self.imp();
        let current = imp.current_step.get();
        if current > 0 {
            imp.current_step.set(current - 1);
            self.show_current_step();
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
