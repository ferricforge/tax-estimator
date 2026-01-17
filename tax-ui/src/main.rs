mod state;
mod views;

use cursive::event::Key;
use cursive::Cursive;
use state::AppState;

fn main() {
    // Initialize Cursive with the crossterm backend
    let mut siv = cursive::crossterm();

    // Initialize application state with current tax year
    siv.set_user_data(AppState::new(2025));

    // Set up global key bindings
    setup_global_callbacks(&mut siv);

    // Display the main menu
    views::show_main_menu(&mut siv);

    // Start the event loop
    siv.run();
}

fn setup_global_callbacks(siv: &mut Cursive) {
    // Allow quitting with Ctrl+Q from anywhere
    siv.add_global_callback(cursive::event::Event::CtrlChar('q'), |s| s.quit());

    // Esc pops a layer or quits if at root
    siv.add_global_callback(Key::Esc, |s| {
        if s.screen().len() > 1 {
            s.pop_layer();
        } else {
            s.quit();
        }
    });
}
