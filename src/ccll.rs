use anyhow::Result;
use base64::prelude::{Engine, BASE64_URL_SAFE};
use flate2::{read::ZlibDecoder, Decompress};
use std::{fs, io::Read, path::Path};

pub fn read_ccll_file(path: &Path) -> Result<String> {
	let mut raw = fs::read(path)?;
	let encoded = raw.get(0) == Some(&67);
	if encoded {
		raw.iter_mut().for_each(|i| *i ^= 0xB);
	}
	let data = std::str::from_utf8(&raw)?
		.trim_end_matches('\0')
		.to_string();

	if encoded {
		decode_level_data(&data)
	} else {
		Ok(data)
	}
}

pub fn decode_level_data(data: &str) -> Result<String> {
	let decoded = &BASE64_URL_SAFE.decode(data)?[10..];
	let mut decompressor =
		ZlibDecoder::new_with_decompress(decoded, Decompress::new_with_window_bits(false, 15));
	let mut decompressed = String::new();
	decompressor.read_to_string(&mut decompressed)?;
	Ok(decompressed)
}
