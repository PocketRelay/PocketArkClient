use native_dialog::MessageDialog;

pub mod iced;

/// Shows a native info dialog with the provided title and text
///
/// `title` The title of the dialog
/// `text`  The text of the dialog
pub fn show_info(title: &str, text: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Info)
        .show_alert()
        .unwrap()
}

/// Shows a native error dialog with the provided title and text
///
/// `title` The title of the dialog
/// `text`  The text of the dialog
pub fn show_error(title: &str, text: &str) {
    MessageDialog::new()
        .set_title(title)
        .set_text(text)
        .set_type(native_dialog::MessageType::Error)
        .show_alert()
        .unwrap()
}
