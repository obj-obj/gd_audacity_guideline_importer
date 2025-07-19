#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum GuidelineColor {
	Green,
	Orange,
	Yellow,
}

impl GuidelineColor {
	pub fn value(&self) -> &'static str {
		match self {
			Self::Orange => "0",
			Self::Yellow => "0.9",
			Self::Green => "1",
		}
	}
}
