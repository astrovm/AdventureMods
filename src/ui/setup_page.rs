use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::setup::{common, sa2, sadx, steps};
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
        pub installer_path: RefCell<Option<std::path::PathBuf>>,
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
        obj.imp().current_step.set(0);
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
                imp.next_button.set_label("Continue");
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
                let installer_path = imp.installer_path.borrow().clone();
                action_button.connect_clicked(move |btn| {
                    btn.set_sensitive(false);
                    if let Some(ref game) = game {
                        Self::run_external_action(
                            step_id,
                            game.clone(),
                            installer_path.clone(),
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

                let scrolled = gtk::ScrolledWindow::builder()
                    .hscrollbar_policy(gtk::PolicyType::Never)
                    .vscrollbar_policy(gtk::PolicyType::Automatic)
                    .max_content_height(300)
                    .propagate_natural_height(true)
                    .build();

                let list_box = gtk::Box::builder()
                    .orientation(gtk::Orientation::Vertical)
                    .spacing(6)
                    .build();

                let mut selected = Vec::new();
                for (i, mod_entry) in sa2::RECOMMENDED_MODS.iter().enumerate() {
                    let check = gtk::CheckButton::builder()
                        .label(format!("{} — {}", mod_entry.name, mod_entry.description))
                        .active(true)
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

                    list_box.append(&check);
                    selected.push(i);
                }
                imp.selected_mods.replace(selected);

                scrolled.set_child(Some(&list_box));
                content_box.append(&scrolled);
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
        installer_path: Option<std::path::PathBuf>,
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
                "run_installer" => {
                    if let Some(path) = installer_path {
                        sadx::run_installer(&path).await
                    } else {
                        Err(anyhow::anyhow!(
                            "Installer not found. Go back and re-download."
                        ))
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

            let progress_fn: Option<crate::external::download::ProgressFn> =
                Some(Box::new(move |dl, total| {
                    let _ = tx.send_blocking((dl, total));
                }));

            let result = match step_id {
                "download_installer" => {
                    let temp = std::env::temp_dir().join("adventure-mods");
                    match sadx::download_installer(&temp, progress_fn).await {
                        Ok(path) => {
                            obj.imp().installer_path.replace(Some(path));
                            Ok(())
                        }
                        Err(e) => Err(e),
                    }
                }
                "install_mod_manager" => {
                    sa2::install_mod_manager(&game.path, progress_fn).await
                }
                "download_mods" => {
                    let selected: Vec<usize> = obj.imp().selected_mods.borrow().clone();
                    let total = selected.len();
                    for (i, idx) in selected.iter().enumerate() {
                        if cancel_flag.load(Ordering::Relaxed) {
                            return; // Cancelled — show_current_step already called
                        }
                        if let Some(mod_entry) = sa2::RECOMMENDED_MODS.get(*idx) {
                            progress_bar.set_text(Some(&format!(
                                "[{}/{}] {}",
                                i + 1,
                                total,
                                mod_entry.name,
                            )));
                            progress_bar.set_fraction((i as f64) / (total as f64));
                            if let Err(e) = sa2::install_mod(&game.path, mod_entry, None).await {
                                tracing::error!("Failed to install {}: {e}", mod_entry.name);
                                obj.show_error(&format!(
                                    "Failed to install {}: {e}",
                                    mod_entry.name
                                ));
                                return;
                            }
                        }
                    }
                    Ok(())
                }
                _ => Ok(()),
            };

            if cancel_flag.load(Ordering::Relaxed) {
                return; // Was cancelled
            }

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
            imp.current_step.set(next);
            self.show_current_step();
        }
    }

    fn on_next_clicked(&self) {
        if self.imp().is_error.get() {
            // Retry: re-run the current step
            self.show_current_step();
        } else {
            self.advance_step();
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
