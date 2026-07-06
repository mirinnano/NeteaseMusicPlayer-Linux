//
// now_playing.rs
// Apple Music-style full window now playing view
//

use gtk::{gdk, gio, glib, prelude::*, subclass::prelude::*};
use adw::subclass::prelude::*;
use std::sync::atomic::Ordering;

use crate::application::Action;
use crate::model::ImageDownloadImpl;
use crate::path::CACHE;
use async_channel::Sender;

mod imp {
    use super::*;

    #[derive(Debug, gtk::CompositeTemplate)]
    #[template(resource = "/com/gitee/gmg137/NeteaseCloudMusicGtk4/gtk/now-playing-view.ui")]
    pub struct NowPlayingView {
        #[template_child]
        pub background_picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub album_art_picture: TemplateChild<gtk::Picture>,
        #[template_child]
        pub song_title_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub artist_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub back_button: TemplateChild<gtk::Button>,
        #[template_child]
        pub lyrics_container: TemplateChild<gtk::Box>,
        #[template_child]
        pub lyrics_scroll: TemplateChild<gtk::ScrolledWindow>,

        pub sender: std::sync::OnceLock<Sender<Action>>,
        pub current_lyrics: std::sync::RwLock<Vec<(u64, String)>>,
        pub current_lyric_index: std::sync::atomic::AtomicUsize,
    }

    impl Default for NowPlayingView {
        fn default() -> Self {
            Self {
                background_picture: Default::default(),
                album_art_picture: Default::default(),
                song_title_label: Default::default(),
                artist_label: Default::default(),
                back_button: Default::default(),
                lyrics_container: Default::default(),
                lyrics_scroll: Default::default(),
                sender: Default::default(),
                current_lyrics: Default::default(),
                current_lyric_index: std::sync::atomic::AtomicUsize::new(usize::MAX),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for NowPlayingView {
        const NAME: &'static str = "NowPlayingView";
        type Type = super::NowPlayingView;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            let provider = gtk::CssProvider::new();
            provider.load_from_resource("/com/gitee/gmg137/NeteaseCloudMusicGtk4/themes/now-playing.css");
            if let Some(display) = gdk::Display::default() {
                gtk::style_context_add_provider_for_display(
                    &display,
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for NowPlayingView {}
    impl WidgetImpl for NowPlayingView {}
    impl BinImpl for NowPlayingView {}
}

glib::wrapper! {
    pub struct NowPlayingView(ObjectSubclass<imp::NowPlayingView>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl NowPlayingView {
    pub fn new() -> Self {
        glib::Object::new()
    }

    pub fn set_sender(&self, sender: Sender<Action>) {
        let imp = self.imp();
        imp.sender.set(sender.clone()).unwrap();
        let close_sender = sender;
        imp.back_button.connect_clicked(move |_| {
            close_sender.send_blocking(Action::CloseNowPlayingView).unwrap();
        });
    }

    pub fn update_song(&self, title: &str, artist: &str, album_id: u64, pic_url: &str) {
        let imp = self.imp();
        imp.song_title_label.set_label(title);
        imp.artist_label.set_label(artist);

        if !pic_url.is_empty() {
            let sender = match imp.sender.get() {
                Some(s) => s.clone(),
                None => return,
            };

            let mut path_cover = CACHE.clone();
            path_cover.push(format!("{}-nowplaying.jpg", album_id));

            let album_art = imp.album_art_picture.get();
            let bg_pic = imp.background_picture.clone();

            if path_cover.exists() {
                Self::load_album_art_from_cache(&album_art, &bg_pic, &path_cover);
            } else {
                let album_art_weak = glib::SendWeakRef::from(album_art.downgrade());
                let bg_pic_weak = glib::SendWeakRef::from(bg_pic.downgrade());
                let cache_path = path_cover.clone();
                sender
                    .send_blocking(Action::DownloadImage(
                        pic_url.to_owned(),
                        path_cover.to_owned(),
                        320,
                        320,
                        Some(std::sync::Arc::new(move |_| {
                            if let Some(art) = album_art_weak.upgrade() {
                                if let Some(bg) = bg_pic_weak.upgrade() {
                                    Self::load_album_art_from_cache(&art, &bg, &cache_path);
                                }
                            }
                        })),
                    ))
                    .unwrap();
            }
        }
    }

    fn load_album_art_from_cache(album_art: &gtk::Picture, bg_pic: &gtk::Picture, path: &std::path::Path) {
        if let Ok(pixbuf) = gtk::gdk_pixbuf::Pixbuf::from_file(path) {
            if let Some(scaled) = pixbuf.scale_simple(320, 320, gtk::gdk_pixbuf::InterpType::Bilinear) {
                album_art.set_pixbuf(Some(&scaled));
            }
            let w = 64;
            let h = (pixbuf.height() as f64 * w as f64 / pixbuf.width() as f64) as i32;
            if let Some(small) = pixbuf.scale_simple(w, h, gtk::gdk_pixbuf::InterpType::Bilinear) {
                bg_pic.set_paintable(Some(&gdk::Texture::for_pixbuf(&small)));
            }
        }
    }

    pub fn set_playing(&self, _playing: bool) {}

    fn filter_lyrics(lrc: Vec<(u64, String)>) -> Vec<(u64, String)> {
        const METADATA: &[&str] = &[
            "作词", "作曲", "编曲", "制作人", "吉他", "贝斯", "鼓", "弦乐",
            "和声", "录音", "混音", "母带", "OP", "SP", "制作", "原曲",
            "原唱", "翻唱", "改编", "演唱", "演奏", "监制", "发行", "出品",
            "词：", "曲：", "编曲：", "制作人：",
            "词:", "曲:", "编曲:", "制作人:",
        ];
        let mut out = Vec::with_capacity(lrc.len());
        let mut prev_ts = None;
        for (ts, text) in lrc {
            let t = text.trim();
            if t.is_empty() || METADATA.iter().any(|p| t.starts_with(p)) || prev_ts == Some(ts) {
                continue;
            }
            prev_ts = Some(ts);
            out.push((ts, text));
        }
        out
    }

    pub fn update_lyrics(&self, lrc: Vec<(u64, String)>) {
        let imp = self.imp();
        let filtered = Self::filter_lyrics(lrc);
        *imp.current_lyrics.write().unwrap() = filtered.clone();

        // Clear existing labels
        let container = imp.lyrics_container.get();
        let mut to_remove = vec![];
        let mut child = container.first_child();
        while let Some(c) = child {
            to_remove.push(c.clone());
            child = c.next_sibling();
        }
        for c in &to_remove {
            container.remove(c);
        }

        // Add lyrics lines with click-to-seek
        if let Some(sender) = imp.sender.get() {
            for (ts, text) in &filtered {
                let label = gtk::Label::new(Some(text));
                label.add_css_class("lyric-line");
                label.set_halign(gtk::Align::Start);
                label.set_xalign(0.0);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);

                let seek_sender = sender.clone();
                let seek_ts = *ts;
                let gesture = gtk::GestureClick::new();
                gesture.connect_pressed(move |_, _, _, _| {
                    let _ = seek_sender.send_blocking(Action::SeekTo(seek_ts * 1000));
                });
                label.add_controller(gesture);
                container.append(&label);
            }
        }
        imp.current_lyric_index.store(usize::MAX, Ordering::Relaxed);
    }

    pub fn update_lyrics_highlight(&self, time_ms: u64) {
        let imp = self.imp();
        let lyrics = imp.current_lyrics.read().unwrap();
        if lyrics.is_empty() {
            return;
        }

        let idx = match crate::gui::playlist_lyrics::get_playing_indexes(lyrics.clone(), time_ms) {
            Some((i, _)) => i,
            None => return,
        };

        if idx == imp.current_lyric_index.swap(idx, Ordering::Relaxed) {
            return;
        }

        // Collect children
        let container = imp.lyrics_container.get();
        let mut children = vec![];
        let mut child = container.first_child();
        while let Some(c) = child {
            children.push(c.clone());
            child = c.next_sibling();
        }

        // Update CSS classes
        for (i, child) in children.iter().enumerate() {
            if let Some(label) = child.downcast_ref::<gtk::Label>() {
                if i == idx {
                    label.add_css_class("lyric-line-active");
                    label.remove_css_class("lyric-line-past");
                } else if i < idx {
                    label.remove_css_class("lyric-line-active");
                    label.add_css_class("lyric-line-past");
                } else {
                    label.remove_css_class("lyric-line-active");
                    label.remove_css_class("lyric-line-past");
                }
            }
        }

        // Scroll active lyric to ~1/3 from top
        if let Some(active) = children.get(idx) {
            let adj = imp.lyrics_scroll.vadjustment();
            let alloc = active.allocation();
            let target = (alloc.y() as f64 - adj.page_size() / 3.0)
                .clamp(0.0, adj.upper() - adj.page_size())
                .max(0.0);
            adj.set_value(target);
        }
    }
}

impl Default for NowPlayingView {
    fn default() -> Self {
        Self::new()
    }
}
