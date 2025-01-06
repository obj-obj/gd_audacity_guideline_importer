use anyhow::Result;
use base64::prelude::{Engine, BASE64_URL_SAFE};
use clap::Parser;
use constcat::concat;
use core::{panic, str};
use fancy_regex::{Match, Regex};
use flate2::{read::ZlibDecoder, Decompress};
use std::{
	io::{stdin, stdout, Read, Write},
	path::PathBuf,
};

fn decode_level_data(data: &str) -> Result<String> {
	// First 10 characters of the decoded file are always invalid, for some reason. Maybe metadata?
	let decoded = &BASE64_URL_SAFE.decode(data)?[10..];
	let mut decompressor =
		ZlibDecoder::new_with_decompress(decoded, Decompress::new_with_window_bits(false, 15));
	let mut decompressed = String::new();
	decompressor.read_to_string(&mut decompressed)?;
	Ok(decompressed)
}

fn regex_to_vec(regex: Regex, data: &str) -> Result<Vec<Match>> {
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

#[derive(Parser)]
struct Cli {
	/// Path to the Audacity labels file
	#[arg(long)]
	labels_file: PathBuf,
	/// Optional level to modify. If unset, level name is asked for at runtime
	#[arg(long)]
	level_name: Option<String>,
	/// Prints the modified save data instead of writing to CCLocalLevels.dat
	#[arg(long)]
	dry_run: bool,
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
	let ccl_location = {
		#[cfg(target_os = "linux")]
		{ home_dir.join(LINUX_PATH) }
		#[cfg(target_os = "windows")]
		{ home_dir.join(WINDOWS_PATH) }
		#[cfg(not(any(target_os = "windows", target_os = "linux")))]
		{ compile_error!("Only Linux and Windows is supported") }
	};

	let mut ccll_raw = std::fs::read(&ccl_location)?;
	let encoded = ccll_raw[0] == 67;
	if encoded {
		ccll_raw.iter_mut().for_each(|i| *i ^= 0xB);
	}
	let ccll_data = str::from_utf8(&ccll_raw)?.trim_end_matches('\0');

	let mut cc_local_levels = if encoded {
		decode_level_data(ccll_data)?
	} else {
		ccll_data.to_string()
	};
	let labels_data = std::fs::read_to_string(&cli.labels_file)?;

	// Please forgive me for what I'm about to do
	// TODO don't use fucking regex to parse XML
	let level_names = regex_to_vec(
		Regex::new("(?<=<s>)[^<>=]+(?=</s><k>k4</k>)")?,
		&cc_local_levels,
	)?;

	let level_name = if let Some(level_name) = cli.level_name {
		level_name
	} else {
		list_levels(&level_names);
		print!("Select a level #: ");
		stdout().flush()?;
		let mut input = String::new();
		stdin().read_line(&mut input)?;
		input
	};
	let level_index = level_names
		.iter()
		.position(|i| i.as_str() == level_name)
		.expect("Invalid level name");

	let level_data_match = regex_to_vec(
		Regex::new("(?<=<k>k4</k><s>)[^<>]+(?=</s>)")?,
		&cc_local_levels,
	)?[level_index];
	let level_data_str = level_data_match.as_str();
	let mut level_data = if level_data_str.contains('|') {
		level_data_str.to_string()
	} else {
		decode_level_data(level_data_str)?
	};

	let guidelines_match = Regex::new("(?<=kA14,)[0-9.~]*")?
		.find(&level_data)?
		.unwrap();

	let mut labels: Vec<(f64, &str)> = vec![];
	for line in labels_data.lines() {
		// TODO actually handle invalid input
		if line.is_empty() {
			continue;
		}
		let last = line.chars().last().unwrap();
		let time = line.split('\t').next().unwrap();

		labels.push((
			time.parse()?,
			match last.to_digit(3).unwrap_or(0) {
				0 => "0.9", // Yellow
				1 => "1",   // Green
				2 => "0",   // Orange
				_ => panic!("This shouldn't be possible"),
			},
		));
	}
	// Is there a better way to do this?
	labels.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

	let mut new_guidelines = String::new();
	for label in labels {
		new_guidelines += &format!("{}~{}~", label.0, label.1);
	}

	// TODO recompress save data with zlib so it doesn't take up extra disk space until the next time the game is launched
	// Geometry Dash also seems to be doing some weird stuff when not given a precompressed save file (inserting null characters into the file, etc)
	level_data.replace_range(guidelines_match.range(), &new_guidelines);
	cc_local_levels.replace_range(level_data_match.range(), &level_data);

	if cli.dry_run {
		println!("---New guideline string---\n{new_guidelines}\n");
		println!("---CCLocalLevels.dat---\n{cc_local_levels}");
	} else {
		std::fs::write(&ccl_location, &cc_local_levels)?;
	}

	Ok(())
}
