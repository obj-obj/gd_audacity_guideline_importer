use anyhow::Result;
use base64::prelude::{Engine, BASE64_URL_SAFE};
use clap::Parser;
use compact_str::format_compact;
use constcat::concat;
use core::{panic, str};
use fancy_regex::{Match, Regex};
use flate2::{read::ZlibDecoder, Decompress};
use ordered_float::NotNan;
use rfd::FileDialog;
use slint::{ModelRc, SharedString, ToSharedString, VecModel};
use std::{
	cell::RefCell,
	fs,
	io::{stdin, stdout, Read, Write},
	path::PathBuf,
	rc::Rc,
};
slint::include_modules!();

#[derive(Parser)]
struct Cli {
	/// Path to the Audacity labels file
	#[arg(long)]
	labels_file: Option<PathBuf>,
	/// Optional level to modify. If unset, level name is asked for at runtime
	#[arg(long)]
	level_name: Option<String>,
	/// Prints the modified save data instead of writing to CCLocalLevels.dat
	#[arg(long)]
	dry_run: bool,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
enum GuidelineColor {
	Orange,
	Yellow,
	Green,
}
impl GuidelineColor {
	/// Converts to the color value used in save data
	fn value(&self) -> &str {
		match self {
			Self::Orange => "0",
			Self::Yellow => "0.9",
			Self::Green => "1",
		}
	}
}

fn decode_level_data(data: &str) -> Result<String> {
	// First 10 characters of the decoded file are always invalid, for some reason. Maybe metadata?
	let decoded = &BASE64_URL_SAFE.decode(data)?[10..];
	let mut decompressor =
		ZlibDecoder::new_with_decompress(decoded, Decompress::new_with_window_bits(false, 15));
	let mut decompressed = String::new();
	decompressor.read_to_string(&mut decompressed)?;
	Ok(decompressed)
}

fn regex_to_vec(regex: Regex, data: &str) -> Result<Vec<Match<'_>>> {
	let mut matches = vec![];
	for capture_match in regex.captures_iter(data) {
		for sub_capture_match in capture_match?.iter() {
			matches.push(sub_capture_match.unwrap());
		}
	}
	Ok(matches)
}

fn list_levels(level_names: &[Match]) {
	println!("Level names:");
	for (i, name) in level_names.iter().enumerate() {
		println!("{i}: {}", name.as_str());
	}
}

const WINDOWS_PATH: &str = "AppData/Local/GeometryDash/CCLocalLevels.dat";
const LINUX_PATH: &str = concat!(
	".steam/steam/steamapps/compatdata/322170/pfx/drive_c/users/steamuser/",
	WINDOWS_PATH
);

fn main() -> Result<()> {
	let cli = Cli::parse();

	let home_dir = dirs::home_dir().unwrap();
	#[rustfmt::skip]
	// TODO better way to do this
	let save_file = {
		#[cfg(target_os = "linux")]
		{ home_dir.join(LINUX_PATH) }
		#[cfg(target_os = "windows")]
		{ home_dir.join(WINDOWS_PATH) }
		#[cfg(not(any(target_os = "windows", target_os = "linux")))]
		{ compile_error!("Only Linux and Windows is supported") }
	};

	let mut save_data = fs::read(&save_file)?;
	let encoded = save_data[0] == 67;
	if encoded {
		save_data.iter_mut().for_each(|i| *i ^= 0xB);
	}
	// Sometimes the save data has null characters at the end for some reason
	let save_data = str::from_utf8(&save_data)?.trim_end_matches('\0');
	let mut save_data = if encoded {
		decode_level_data(save_data)?
	} else {
		save_data.to_string()
	};

	// TODO don't use regex to parse XML
	let level_names = regex_to_vec(Regex::new("(?<=<s>)[^<>=]+(?=</s><k>k4</k>)")?, &save_data)?;

	let labels_data = if let Some(labels_file) = &cli.labels_file {
		std::fs::read_to_string(labels_file)?
	} else {
		let level_names = level_names
			.iter()
			.map(|i| SharedString::from(i.as_str()))
			.collect();
		return ui(save_file, save_data, level_names);
	};

	// Everything past this point is CLI

	let level_index = if let Some(level_name) = cli.level_name {
		level_names
			.iter()
			.position(|i| i.as_str() == level_name)
			.expect("Invalid level name")
	} else {
		list_levels(&level_names);
		print!("Select a level #: ");
		stdout().flush()?;
		let mut input = String::new();
		stdin().read_line(&mut input)?;
		let index = input.trim().parse()?;
		if level_names.get(index).is_none() {
			panic!("Invalid level #");
		}
		index
	};

	let new_guidelines = create_guidelines(&labels_data);
	modify_save_data(level_index, &new_guidelines, &mut save_data)?;

	// TODO recompress save data with zlib so it doesn't take up extra disk space until the next time the game is launched
	// Geometry Dash also seems to be doing some weird stuff when not given a precompressed save file (inserting null characters into the file, etc)

	if cli.dry_run {
		println!("---New guideline string---\n{new_guidelines}\n");
		println!("---CCLocalLevels.dat---\n{save_data}");
	} else {
		std::fs::write(save_file, save_data)?;
	}

	Ok(())
}

fn ui(
	save_file: PathBuf,
	mut save_data: String,
	level_names: VecModel<SharedString>,
) -> Result<()> {
	let ui_local = Rc::new(MainWindow::new()?);
	let labels_file = Box::leak(Box::new(RefCell::new(None)));

	ui_local.set_level_names(ModelRc::from(Rc::new(level_names)));

	{
		let ui = ui_local.clone();
		let labels_file = labels_file.clone();
		ui_local.on_choose_labels_file(move || {
			let file = FileDialog::new().add_filter("Text", &["txt"]).pick_file();
			ui.set_file_name(if let Some(file) = &file {
				labels_file.replace(Some(std::fs::read_to_string(file).unwrap()));
				format!(": {}", file.display()).to_shared_string()
			} else {
				"".to_shared_string()
			});
		});
	}

	let ui = ui_local.clone();
	ui_local.on_apply_guidelines(move || {
		let new_guidelines = if let Some(labels_data) = &*labels_file.borrow() {
			create_guidelines(labels_data)
		} else {
			ui.set_status("Error: No labels file selected".to_shared_string());
			return;
		};

		let level_index = ui.get_level_index();
		let level_index = if level_index == -1 {
			ui.set_status("Error: No level selected".to_shared_string());
			return;
		} else {
			level_index as usize
		};

		if let Err(e) = modify_save_data(level_index, &new_guidelines, &mut save_data) {
			ui.set_status(format!("Error adding guidelines: {e}").to_shared_string());
			return;
		}

		if let Err(e) = std::fs::write(&save_file, &save_data) {
			ui.set_status(format!("Error writing save data: {e}").to_shared_string());
			return;
		}

		ui.set_status("Applied guidelines".to_shared_string());
	});

	ui_local.run()?;
	Ok(())
}

fn create_guidelines(labels_data: &str) -> String {
	let labels: Vec<(NotNan<f64>, GuidelineColor)> = labels_data
		.lines()
		.filter(|line| !line.is_empty())
		.map(|line| {
			// So all possible label names (or a missing one) resolve to a valid color
			let color = match line.chars().last().unwrap().to_digit(3).unwrap_or(0) {
				0 => GuidelineColor::Yellow,
				1 => GuidelineColor::Green,
				2 => GuidelineColor::Orange,
				_ => std::unreachable!(), // Radix is 3 so value will never be above 2
			};
			// The time is always followed by a tab
			let time = line.split('\t').next().unwrap();
			(time.parse().unwrap(), color)
		})
		.collect();

	labels.iter().fold(String::new(), |acc, label| {
		acc + &format_compact!("{:.6}~{}~", label.0, label.1.value())
	})
}

fn modify_save_data(
	level_index: usize,
	new_guidelines: &str,
	save_data: &mut String,
) -> Result<()> {
	let level_data_match =
		regex_to_vec(Regex::new("(?<=<k>k4</k><s>)[^<>]+(?=</s>)")?, save_data)?[level_index];
	let level_data = level_data_match.as_str();
	// If the level data contains a `|` character, it isn't encoded
	let mut level_data = if level_data.contains('|') {
		level_data.to_string()
	} else {
		decode_level_data(level_data)?
	};

	let guidelines_match = Regex::new("(?<=kA14,)[0-9.~]*")?
		.find(&level_data)?
		.unwrap();

	level_data.replace_range(guidelines_match.range(), new_guidelines);
	save_data.replace_range(level_data_match.range(), &level_data);

	Ok(())
}
