use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

use crate::steam::game::Game;

mod imp {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/game_card.ui")]
    pub struct AdventureModsGameCard {
        #[template_child]
        pub title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub status_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub setup_button: TemplateChild<gtk::Button>,
        pub setup_callback: RefCell<Option<Box<dyn Fn()>>>,
    }

    impl std::fmt::Debug for AdventureModsGameCard {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.debug_struct("AdventureModsGameCard").finish()
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsGameCard {
        const NAME: &'static str = "AdventureModsGameCard";
        type Type = super::AdventureModsGameCard;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AdventureModsGameCard {
        fn constructed(&self) {
            self.parent_constructed();

            let obj = self.obj().clone();
            self.setup_button.connect_clicked(move |_| {
                let imp = obj.imp();
                if let Some(ref cb) = *imp.setup_callback.borrow() {
                    cb();
                }
            });
        }
    }

    impl WidgetImpl for AdventureModsGameCard {}
    impl BoxImpl for AdventureModsGameCard {}
}

glib::wrapper! {
    pub struct AdventureModsGameCard(ObjectSubclass<imp::AdventureModsGameCard>)
        @extends gtk::Widget, gtk::Box,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl AdventureModsGameCard {
    pub fn new(game: &Game) -> Self {
        let obj: Self = glib::Object::builder().build();
        let imp = obj.imp();

        imp.title_label.set_label(game.kind.name());
        imp.status_label.set_label(&format!(
            "Found at {}",
            game.path.display()
        ));
        imp.status_label.add_css_class("game-card-status-installed");

        obj
    }

    pub fn connect_setup_clicked<F: Fn() + 'static>(&self, callback: F) {
        self.imp()
            .setup_callback
            .replace(Some(Box::new(callback)));
    }
}
