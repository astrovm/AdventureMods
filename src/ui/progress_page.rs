use adw::prelude::*;
use adw::subclass::prelude::*;
use gtk::glib;

mod imp {
    use super::*;

    #[derive(Debug, Default, gtk::CompositeTemplate)]
    #[template(resource = "/io/github/astrovm/AdventureMods/resources/ui/progress_page.ui")]
    pub struct AdventureModsProgressPage {
        #[template_child]
        pub progress_title: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress_status: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub cancel_button: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for AdventureModsProgressPage {
        const NAME: &'static str = "AdventureModsProgressPage";
        type Type = super::AdventureModsProgressPage;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for AdventureModsProgressPage {}
    impl WidgetImpl for AdventureModsProgressPage {}
    impl BinImpl for AdventureModsProgressPage {}
}

glib::wrapper! {
    pub struct AdventureModsProgressPage(ObjectSubclass<imp::AdventureModsProgressPage>)
        @extends gtk::Widget, adw::Bin;
}

impl AdventureModsProgressPage {
    pub fn new() -> Self {
        glib::Object::builder().build()
    }

    pub fn set_title(&self, title: &str) {
        self.imp().progress_title.set_label(title);
    }

    pub fn set_status(&self, status: &str) {
        self.imp().progress_status.set_label(status);
    }

    pub fn set_fraction(&self, fraction: f64) {
        self.imp().progress_bar.set_fraction(fraction);
    }

    pub fn set_progress_text(&self, text: &str) {
        self.imp().progress_bar.set_text(Some(text));
    }

    pub fn pulse(&self) {
        self.imp().progress_bar.pulse();
    }

    pub fn connect_cancel<F: Fn() + 'static>(&self, callback: F) {
        self.imp().cancel_button.connect_clicked(move |_| {
            callback();
        });
    }
}
