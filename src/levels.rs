use crate::types::GuidelineColor;
use anyhow::Result;
use fancy_regex::Regex;

pub fn extract_level_names(data: &str) -> Result<Vec<String>> {
	let regex = Regex::new("(?<=<s>)[^<>=]+(?=</s><k>k4</k>)")?;
	Ok(regex
		.captures_iter(data)
		.filter_map(|cap| cap.ok()?.get(0).map(|m| m.as_str().to_string()))
		.collect())
}

pub fn replace_guidelines(
	level_data: &str,
	labels: &[(ordered_float::NotNan<f64>, GuidelineColor)],
) -> Result<String> {
	let guidelines_regex = Regex::new("(?<=kA14,)[0-9.~]*")?;
	let guidelines_match = guidelines_regex
		.find(level_data)?
		.ok_or_else(|| anyhow::anyhow!("Guidelines not found"))?;

	let new_guidelines = labels.iter().fold(String::new(), |mut acc, (time, color)| {
		acc += &format!("{:.6}~{}~", time, color.value());
		acc
	});

	let mut updated = level_data.to_string();
	updated.replace_range(guidelines_match.range(), &new_guidelines);
	Ok(updated)
}
