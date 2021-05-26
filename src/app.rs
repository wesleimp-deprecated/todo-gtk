use crate::utils::spawn;
use crate::widgets::Task;
use crate::{BgEvent, Event, TaskEntity};

use async_channel::Sender;
use glib::clone;
use glib::SourceId;
use gtk::prelude::*;
use slotmap::SlotMap;

pub struct App {
    pub container: gtk::Grid,
    pub tasks: SlotMap<TaskEntity, Task>,
    pub scheduled_write: Option<SourceId>,
    pub tx: Sender<Event>,
    pub btx: Sender<BgEvent>,
}

impl App {
    pub fn new(app: &gtk::Application, tx: Sender<Event>, btx: Sender<BgEvent>) -> Self {
        let container = cascade! {
            gtk::Grid::new();
            ..set_column_spacing(4);
            ..set_row_spacing(4);
            ..set_border_width(4);
            ..show();
        };

        let scrolled = gtk::ScrolledWindowBuilder::new()
            .hscrollbar_policy(gtk::PolicyType::Never)
            .build();

        scrolled.add(&container);

        let _window = cascade! {
            gtk::ApplicationWindow::new(app);
            ..set_title("Todo");
            ..add(&scrolled);
            ..connect_delete_event(clone!(@strong tx, @strong scrolled => move |win, _| {
                // Detach to preserve widgets after destruction of window
                win.remove(&scrolled);

                let tx = tx.clone();
                spawn(async move {
                    let _ = tx.send(Event::Closed).await;
                });
                gtk::Inhibit(false)
            }));
            ..show_all();
        };

        gtk::Window::set_default_icon_name("icon-name-here");

        let mut app = Self {
            container,
            tasks: SlotMap::with_key(),
            scheduled_write: None,
            tx,
            btx,
        };

        app.insert_row(0);

        app
    }

    pub fn clear(&mut self) {
        while let Some(entity) = self.tasks.keys().next() {
            self.remove_(entity);
        }
    }

    pub fn insert(&mut self, entity: TaskEntity) {
        let mut insert_at = 0;

        if let Some(task) = self.tasks.get(entity) {
            insert_at = task.row + 1;
        }

        self.insert_row(insert_at);
    }

    fn insert_row(&mut self, row: i32) -> TaskEntity {
        // Increment the row value of each Task is below the new row
        for task in self.tasks.values_mut() {
            if task.row >= row {
                task.row += 1;
            }
        }

        self.container.insert_row(row);
        let task = Task::new(row);

        self.container.attach(&task.entry, 0, row, 1, 1);
        self.container.attach(&task.insert, 1, row, 1, 1);
        self.container.attach(&task.remove, 2, row, 1, 1);

        task.entry.grab_focus();

        let entity = self.tasks.insert(task);
        self.tasks[entity].connect(self.tx.clone(), entity);
        return entity;
    }

    pub fn load(&mut self, data: String) {
        self.clear();

        for (row, line) in data.lines().enumerate() {
            let entity = self.insert_row(row as i32);
            self.tasks[entity].set_text(line);
        }
    }

    pub fn modified(&mut self) {
        if let Some(id) = self.scheduled_write.take() {
            glib::source_remove(id);
        }

        let tx = self.tx.clone();
        self.scheduled_write = Some(glib::timeout_add_local(5000, move || {
            let tx = tx.clone();
            spawn(async move {
                let _ = tx.send(Event::SyncToDisk).await;
            });

            glib::Continue(false)
        }));
    }

    pub fn remove(&mut self, entity: TaskEntity) {
        if self.tasks.len() == 1 {
            return;
        }
        self.remove_(entity);
    }

    fn remove_(&mut self, entity: TaskEntity) {
        if let Some(removed) = self.tasks.remove(entity) {
            self.container.remove_row(removed.row);

            // Decrement the row value of the tasks that were below the removed row
            for task in self.tasks.values_mut() {
                if task.row > removed.row {
                    task.row -= 1;
                }
            }
        }
    }

    pub async fn closed(&mut self) {
        self.sync_to_disk().await;
        let _ = self.btx.send(BgEvent::Quit).await;
    }

    pub async fn sync_to_disk(&mut self) {
        self.scheduled_write = None;

        let contents = fomat_macros::fomat!(
            for node in self.tasks.values() {
                if node.entry.get_text_length() != 0 {
                    (node.entry.get_text()) "\n"
                }
            }
        );

        let _ = self.btx.send(BgEvent::Save("Task".into(), contents)).await;
    }
}
