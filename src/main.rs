use gd_audacity_guideline_importer::*;
use rfd::FileDialog;
use slint::{SharedString, VecModel};
use std::{cell::RefCell, error::Error, fs, path::PathBuf, rc::Rc};

slint::include_modules!();

fn main() -> Result<(), Box<dyn Error>> {
	let ui = MainWindow::new()?;

	let labels_file = Rc::new(RefCell::new(PathBuf::new()));
	let save_file = Rc::new(RefCell::new(PathBuf::new()));

	// Open Labels File
	let labels_file_clone = Rc::clone(&labels_file);
	let ui_weak = ui.as_weak();
	ui.on_open_labels_file(move || {
		if let Some(file) = open_labels_file() {
			*labels_file_clone.borrow_mut() = file.clone();
			if let Some(ui) = ui_weak.upgrade() {
				ui.set_labelFilePath(file.to_string_lossy().to_string().into());
			}
		}
	});

	// Open Save File
	let save_file_clone = Rc::clone(&save_file);
	let ui_weak = ui.as_weak();
	ui.on_open_save_file(move || {
		if let Some(file) = open_save_file() {
			*save_file_clone.borrow_mut() = file.clone();
			if let Some(ui) = ui_weak.upgrade() {
				ui.set_saveFilePath(file.to_string_lossy().to_string().into());
			}

			if let Ok(level_names) = list_levels(&file) {
				let shared_levels: Vec<SharedString> =
					level_names.into_iter().map(SharedString::from).collect();
				let model = Rc::new(VecModel::from(shared_levels));
				if let Some(ui) = ui_weak.upgrade() {
					ui.set_levelList(model.into());
				}
			}
		}
	});

	// Apply Guidelines
	let save_file_clone = save_file.clone();
	let labels_file_clone = labels_file.clone();

	ui.on_apply_guidelines(move |level_name| {
		let save_path = save_file_clone.borrow().clone();
		let labels_path = labels_file_clone.borrow().clone();

		if save_path.exists() && labels_path.exists() {
			match fs::read_to_string(&labels_path) {
				Ok(labels_data) => {
					match apply_guidelines_to_level(
						&save_path,
						&level_name.to_string(),
						&labels_data,
					) {
						Ok(updated_data) => match fs::write(&save_path, updated_data) {
							Ok(_) => {
								show_status_message(String::from(
									"Guidelines applied successfully",
								));
							}
							Err(e) => {
								show_status_message(format!("Error writing save file: {e}"));
							}
						},
						Err(e) => {
							show_status_message(format!("Error applying guidelines: {e}"));
						}
					}
				}
				Err(e) => {
					show_status_message(format!("Error reading labels file: {e}"));
				}
			}
		} else {
			show_status_message(String::from("Missing save or labels file."));
		}
	});

	ui.run()?;
	Ok(())
}

fn open_save_file() -> Option<PathBuf> {
	FileDialog::new()
		.add_filter("Save files", &["dat"])
		.pick_file()
}

fn open_labels_file() -> Option<PathBuf> {
	FileDialog::new()
		.add_filter("Labels file", &["txt"])
		.pick_file()
}

fn show_status_message(message: String) {
	let status_window = StatusMessage::new().unwrap();
	status_window.set_statusMessage(message.into());

	// Clone just for the close handler
	let status_window_weak = status_window.as_weak();
	status_window.on_close_window(move || {
		if let Some(msg) = status_window_weak.upgrade() {
			msg.hide().unwrap();
		}
	});

	status_window.show().unwrap();
}
