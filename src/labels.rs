use crate::types::GuidelineColor;
use anyhow::Result;
use ordered_float::NotNan;

pub fn parse_labels(content: &str) -> Result<Vec<(NotNan<f64>, GuidelineColor)>> {
	let mut labels = Vec::new();
	for line in content.lines().filter(|l| !l.is_empty()) {
		let last = line.chars().last().unwrap();
		let time = line.split('\t').next().unwrap();
		labels.push((
			time.parse()?,
			match last.to_digit(3).unwrap_or(0) {
				0 => GuidelineColor::Yellow,
				1 => GuidelineColor::Green,
				2 => GuidelineColor::Orange,
				_ => anyhow::bail!("Invalid color digit in label line"),
			},
		));
	}
	labels.sort();
	Ok(labels)
}
