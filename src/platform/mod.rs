pub mod secrets;
mod ui;
mod updater;

pub fn run() {
    if updater::handle_helper_arguments() { return; }
    ui::run();
}
