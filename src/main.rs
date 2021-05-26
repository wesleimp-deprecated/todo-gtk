#[macro_use]
extern crate cascade;

mod app;
mod background;
mod utils;
mod widgets;

use self::app::App;
use gio::prelude::*;
use std::path::PathBuf;

pub const APP_ID: &str = "io.github.wesleimp.todo";

slotmap::new_key_type! {
    pub struct TaskEntity;
}

pub enum Event {
    // Insert a task below the given task, identified by its key
    Insert(TaskEntity),
    // A previous task list has been fetched from a file from the background
    // thread, and it is now our job to display it in our UI.
    Load(String),
    // Signals that an entry was modified, and at some point we should save it
    Modified,
    // Removes the task identified by this entity
    Remove(TaskEntity),
    // Signals that we should collect up the text from each task and pass it
    // to a background thread to save it to a file.
    SyncToDisk,
    // Signals that the window has been closed, so we should clean up and quit
    Closed,
    // Signals that the process has saved to disk and it is safe to exit
    Quit,
}

pub enum BgEvent {
    // Save tasks to a file
    Save(PathBuf, String),

    // Exit the from the event loop
    Quit,
}

fn main() {
    let app_name = "Todo";

    glib::set_program_name(Some(app_name));
    glib::set_application_name(app_name);

    let app = gtk::Application::new(Some(APP_ID), Default::default()).expect("");

    app.connect_activate(|app| {
        let (tx, rx) = async_channel::unbounded();
        let (btx, brx) = async_channel::unbounded();

        std::thread::spawn(glib::clone!(@strong tx => move || {
            // Fetch the executor registered for this thread
            utils::thread_context()
                // Block this thread on an event loop future
                .block_on(background::run(tx, brx));
        }));

        let mut app = App::new(app, tx, btx);

        let event_handler = async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    Event::Modified => app.modified(),
                    Event::Insert(entity) => app.insert(entity),
                    Event::Remove(entity) => app.remove(entity),
                    Event::SyncToDisk => app.sync_to_disk().await,
                    Event::Load(data) => app.load(data),
                    Event::Closed => app.closed().await,
                    Event::Quit => gtk::main_quit(),
                }
            }
        };

        utils::spawn(event_handler);
    });

    app.run(&[]);
}
