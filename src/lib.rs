pub mod ccll;
pub mod labels;
pub mod levels;
pub mod types;

pub use ccll::*;
pub use labels::*;
pub use levels::*;
pub use types::*;

use anyhow::Result;
use std::path::Path;

pub fn list_levels(ccll_path: &Path) -> Result<Vec<String>> {
	let data = ccll::read_ccll_file(ccll_path)?;
	levels::extract_level_names(&data)
}

pub fn apply_guidelines_to_level(
	ccll_path: &Path,
	level_name: &str,
	labels_data: &str,
) -> Result<String> {
	let mut ccll_data = ccll::read_ccll_file(ccll_path)?; // Read the full file content
	let level_names = levels::extract_level_names(&ccll_data)?; // Extract level names

	let level_index = level_names
		.iter()
		.position(|name| name == level_name)
		.ok_or_else(|| anyhow::anyhow!("Level '{}' not found", level_name))?; // Find the level's index

	// Capture the range of the level data within the full ccll_data
	let level_data_full_match = {
		let regex = fancy_regex::Regex::new("(?<=<k>k4</k><s>)[^<>]+(?=</s>)")?;
		regex
			.captures_iter(&ccll_data)
			.nth(level_index)
			.ok_or_else(|| anyhow::anyhow!("Level data not found"))??
			.get(0)
			.ok_or_else(|| anyhow::anyhow!("Level data match failed"))?
	};
	let level_data_range = level_data_full_match.range(); // Store the Range
	let level_data_str = level_data_full_match.as_str(); // Get the string content of the level data

	let current_level_data = if level_data_str.contains('|') {
		level_data_str.to_string()
	} else {
		ccll::decode_level_data(level_data_str)? // Decode if necessary
	};

	let parsed_labels = labels::parse_labels(labels_data)?; // Parse labels
	let updated_level_data = levels::replace_guidelines(&current_level_data, &parsed_labels)?; // Get the modified level data string

	// Now, replace the original level data within the *full* ccll_data
	ccll_data.replace_range(level_data_range, &updated_level_data); //

	Ok(ccll_data) // Return the ENTIRE modified ccll_data string
}
