pub mod secrets;
mod ui;
pub(crate) mod updater;

pub fn run() {
    if updater::handle_helper_arguments() {
        return;
    }
    ui::run();
}
