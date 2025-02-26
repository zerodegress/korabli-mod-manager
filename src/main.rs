use app::iced_main;

mod app;
mod data;
mod error;
mod messages;
mod mod_manager;
mod tasks;

fn main() {
  iced_main().expect("wtf iced")
}
