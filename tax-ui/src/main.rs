mod views;

use cursive::Cursive;

fn main() {
    // Initialize Cursive with the crossterm backend
    let mut siv = cursive::crossterm();

    // Set up global key bindings
    setup_global_callbacks(&mut siv);

    // Display the main menu
    views::show_main_menu(&mut siv);

    // Start the event loop
    siv.run();
}

fn setup_global_callbacks(siv: &mut Cursive) {
    // Allow quitting with 'q' from anywhere (when not in an input field)
    siv.add_global_callback('q', |s| s.quit());

    // Esc quits from main menu level
    siv.add_global_callback(cursive::event::Key::Esc, |s| {
        // If there's more than one layer, pop it; otherwise quit
        if s.screen().len() > 1 {
            s.pop_layer();
        } else {
            s.quit();
        }
    });
}
