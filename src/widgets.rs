use crate::{utils::spawn, Event, TaskEntity};
use async_channel::Sender;
use glib::{clone, SignalHandlerId};
use gtk::prelude::*;

pub struct Task {
    pub entry: gtk::Entry,
    pub insert: gtk::Button,
    pub remove: gtk::Button,

    entry_signal: Option<SignalHandlerId>,

    // Tracks our position in the list
    pub row: i32,
}

impl Task {
    pub fn new(row: i32) -> Self {
        let entry = cascade! {
            gtk::Entry::new();
            ..set_hexpand(true);
            ..show();
        };

        let insert = cascade! {
            gtk::Button::from_icon_name(Some("list-add-symbolic"), gtk::IconSize::Button);
            ..show();
        };

        let remove = cascade! {
            gtk::Button::from_icon_name(Some("list-remove-symbolic"), gtk::IconSize::Button);
            ..show();
        };

        Self {
            insert,
            remove,
            entry,
            entry_signal: None,
            row,
        }
    }

    pub fn connect(&mut self, tx: Sender<Event>, entity: TaskEntity) {
        let signal = self.entry.connect_changed(clone!(@strong tx => move |_| {
            let tx = tx.clone();
            spawn(async move {
                let _ = tx.send(Event::Modified).await;
            });
        }));

        self.entry_signal = Some(signal);

        self.insert
            .connect_clicked(clone!(@strong tx, @weak self.entry as entry => move |_| {
                if entry.get_text_length() == 0 {
                    return;
                }

                let tx = tx.clone();
                spawn(async move {
                    let _ = tx.send(Event::Insert(entity)).await;
                });
            }));

        self.remove.connect_clicked(clone!(@strong tx => move |_| {
            let tx = tx.clone();
            spawn(async move {
                let _ = tx.send(Event::Remove(entity)).await;
            });
        }));

        self.entry
            .connect_activate(clone!(@weak self.entry as entry => move |_| {
                if entry.get_text_length() == 0 {
                    return;
                }

                let tx = tx.clone();
                spawn(async move {
                    let _ = tx.send(Event::Insert(entity)).await;
                });
            }));
    }

    pub fn set_text(&mut self, text: &str) {
        let signal = self.entry_signal.as_ref().unwrap();
        self.entry.block_signal(signal);
        self.entry.set_text(text);
        self.entry.unblock_signal(signal);
    }
}
