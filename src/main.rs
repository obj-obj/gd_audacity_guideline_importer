use anyhow::Result;
use base64::prelude::{Engine, BASE64_URL_SAFE};
use clap::Parser;
use constcat::concat;
use core::panic;
use fancy_regex::{Match, Regex};
use flate2::{read::ZlibDecoder, Decompress};
use std::{
	io::{Read, Write},
	path::PathBuf,
};

fn decode_level_data(mut data: &str) -> Result<String> {
	// Geometry Dash sometimes writes a null character to the end of the file.
	if data.chars().last() == Some('\0') {
		data = &data[..data.len() - 1];
	}

	// First 10 characters of the decoded file are always invalid, for some reason. Maybe metadata?
	let base64_decoded = &BASE64_URL_SAFE.decode(data)?[10..];
	let mut zlib_decoder = ZlibDecoder::new_with_decompress(
		base64_decoded,
		Decompress::new_with_window_bits(false, 15),
	);
	let mut decoded_string = String::new();
	zlib_decoder.read_to_string(&mut decoded_string)?;

	Ok(decoded_string)
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
	/// Path to the Audacity labelt file
	#[arg(long)]
	labels_file: PathBuf,
	/// Lists all levels in CCLocalLevels.dat and exits
	#[arg(long)]
	list_levels: bool,
	/// Optional level to modify. If unset, level name is asked for at runtime
	#[arg(long)]
	level_name: Option<String>,
}

const WINDOWS_PATH: &str = "AppData/Local/GeometryDash/CCLocalLevels.dat";
const LINUX_PATH: &str = concat!(
	".steam/steam/steamapps/compatdata/322170/pfx/drive_c/users/steamuser/",
	WINDOWS_PATH
);

fn main() -> Result<()> {
	let cli = Cli::parse();

	let home_dir = dirs::home_dir().unwrap();
	let windows_location = home_dir.join(WINDOWS_PATH);
	let linux_location = home_dir.join(LINUX_PATH);
	let ccl_location;
	if windows_location.is_file() {
		ccl_location = windows_location;
	} else if linux_location.is_file() {
		ccl_location = linux_location;
	} else {
		panic!("Could not find CCLocalLevels.dat");
	}

	let mut cc_local_levels = decode_level_data(
		&std::fs::read_to_string(&ccl_location)?
			.chars()
			.map(|i| (i as u8 ^ 0xB) as char)
			.collect::<String>(),
	)?;
	let labels_data = std::fs::read_to_string(cli.labels_file)?;

	// Please forgive me for what I'm about to do
	let level_names = regex_to_vec(
		Regex::new("(?<=<s>)[^<>=]+(?=</s><k>k4</k>)")?,
		&cc_local_levels,
	)?;

	if cli.list_levels {
		list_levels(&level_names);
		return Ok(());
	}

	let level_index = if let Some(index) = level_names
		.iter()
		.position(|i| Some(i.as_str()) == cli.level_name.as_ref().map(|i| i.as_str()))
	{
		index
	} else {
		list_levels(&level_names);
		print!("Select a level #: ");
		std::io::stdout().flush()?;
		let mut input = String::new();
		std::io::stdin().read_line(&mut input)?;
		let index: usize = input.trim().parse()?;
		index
	};

	let level_data_match = regex_to_vec(
		Regex::new("(?<=<k>k4</k><s>)[^<>]+=(?=</s>)")?,
		&cc_local_levels,
	)?[level_index];
	let mut level_data = decode_level_data(level_data_match.as_str())?;

	let guidelines_match = Regex::new("(?<=kA14,)[0-9.~]*")?
		.find(&level_data)?
		.unwrap();

	let mut labels: Vec<(f64, u32)> = vec![];
	for line in labels_data.lines() {
		if line == "" {
			continue;
		}
		let last = line.chars().last().unwrap();

		let time = line.splitn(2, '\t').next().unwrap();
		let num = if last == '\t' { '0' } else { last };

		labels.push((time.parse()?, num.to_digit(10).unwrap()));
	}
	labels.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

	let mut new_guidelines = String::new();
	for label in labels {
		new_guidelines += &format!("{}~{}~", label.0, label.1);
	}

	level_data.replace_range(guidelines_match.range(), &new_guidelines);
	cc_local_levels.replace_range(level_data_match.range(), &level_data);

	std::fs::write(&ccl_location, &cc_local_levels)?;

	Ok(())
}
