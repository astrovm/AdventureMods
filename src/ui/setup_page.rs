use std::cell::Cell;

use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::setup::{common, sa2, sadx, steps};
use crate::steam::game::Game;

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
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
        @extends gtk::Widget, adw::Bin;
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

                // Run the auto step
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
                action_button.connect_clicked(move |_| {
                    if let Some(ref game) = game {
                        Self::run_external_action(step_id, game.clone());
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
                content_box.append(&progress_bar);

                self.run_download_step(step.id, progress_bar);
            }
            steps::StepKind::ModSelection => {
                imp.next_button.set_label("Install Selected");
                imp.next_button.set_sensitive(true);

                // Pre-select all mods
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

                    content_box.append(&check);
                    selected.push(i);
                }
                imp.selected_mods.replace(selected);
            }
        }
    }

    fn run_auto_step(&self, step_id: &'static str) {
        let obj = self.clone();
        let game = self.imp().game.borrow().clone();

        glib::spawn_future_local(async move {
            let result = match step_id {
                "check_deps" => {
                    common::ensure_protontricks().await
                }
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
                    // Auto-advance
                    obj.advance_step();
                }
                Err(e) => {
                    obj.show_error(&format!("Error: {e}"));
                }
            }
        });
    }

    fn run_external_action(step_id: &'static str, _game: Game) {
        glib::spawn_future_local(async move {
            match step_id {
                "ge_proton" => {
                    let _ = common::ensure_protonup().await;
                    let _ = common::launch_protonup().await;
                }
                "run_installer" => {
                    // SADX installer launch is handled separately
                }
                _ => {}
            }
        });
    }

    fn run_download_step(&self, step_id: &'static str, progress_bar: gtk::ProgressBar) {
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
                    sadx::download_installer(&temp, progress_fn).await.map(|_| ())
                }
                "install_mod_manager" => {
                    sa2::install_mod_manager(&game.path, progress_fn).await
                }
                "download_mods" => {
                    let selected: Vec<usize> = obj.imp().selected_mods.borrow().clone();
                    let mut result = Ok(());
                    for idx in selected {
                        if let Some(mod_entry) = sa2::RECOMMENDED_MODS.get(idx) {
                            if let Err(e) = sa2::install_mod(&game.path, mod_entry, None).await {
                                tracing::error!("Failed to install {}: {e}", mod_entry.name);
                                result = Err(e);
                            }
                        }
                    }
                    result
                }
                _ => Ok(()),
            };

            match result {
                Ok(()) => {
                    obj.imp().next_button.set_sensitive(true);
                    obj.advance_step();
                }
                Err(e) => {
                    obj.show_error(&format!("Error: {e}"));
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
        self.advance_step();
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
