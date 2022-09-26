use std::{collections::HashMap, str::FromStr};

use axum::http::Uri;
use regex::Regex;

/// Contains the mapping of URIs to redirect to
pub struct UriMappings {
	pub standard: HashMap<String, Uri>,
	pub pattern: Vec<(Regex, String)>,
}

impl UriMappings {
	/// Create a new empty `UriMappings`
	pub fn new(standard: HashMap<String, Uri>, pattern: Vec<(Regex, String)>) -> UriMappings {
		UriMappings { standard, pattern }
	}

	/// Match standard URIs from the collection
	pub fn match_standard(&self, parameter: &str) -> Result<Uri, &str> {
		match self.standard.get(parameter) {
			Some(x) => Ok(x.clone()),
			None => Err("No standard found"),
		}
	}

	/// Match pattern URIs from the collection
	pub fn match_pattern(&self, parameter: &str) -> Result<Uri, &str> {
		for (regex, uri_pattern) in &self.pattern {
			if !regex.is_match(parameter) {
				continue;
			}

			let replacement = regex.replace(parameter, uri_pattern);

			return match Uri::from_str(&replacement) {
				Ok(new_uri) => Ok(new_uri),
				Err(_) => Err("Pattern did not create URI"),
			};
		}

		Err("No pattern found")
	}

	/// Match both standard and pattern URIs from the collection.
	/// Standard URIs will match before patterns
	pub fn match_anything(&self, parameter: &str) -> Result<Uri, &str> {
		match self.match_standard(parameter) {
			Ok(standard) => Ok(standard),
			Err(_) => self.match_pattern(parameter),
		}
	}
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unnecessary_wraps)]

	use axum::http::uri::InvalidUri;

	use super::*;

	#[test]
	fn redirect_standard_uris() -> Result<(), InvalidUri> {
		let standard = HashMap::from([
			("test".to_string(), Uri::from_str("https://example.com")?),
			("1/1".to_string(), Uri::from_str("https://example.com/1")?),
			("3.14".to_string(), Uri::from_str("https://example.com/pi")?),
		]);
		let uri_mappings = UriMappings::new(standard, Vec::new());

		// No matches
		assert!(uri_mappings.match_standard("/invalid").is_err());

		// Can't match an invalid URI, because it must be a URI to be loaded into the hashmap

		// Standard matches
		assert_eq!(
			uri_mappings.match_standard("test").unwrap(),
			Uri::from_str("https://example.com")?
		);
		assert_eq!(
			uri_mappings.match_standard("1/1").unwrap(),
			Uri::from_str("https://example.com/1")?
		);
		assert_eq!(
			uri_mappings.match_standard("3.14").unwrap(),
			Uri::from_str("https://example.com/pi")?
		);

		Ok(())
	}

	#[test]
	fn redirect_pattern_uris() -> Result<(), InvalidUri> {
		let pattern = vec![
			(
				Regex::new(r"(?P<last>[^,\s]+),\s+(?P<first>\S+)").unwrap(),
				"$first $last".to_string(),
			),
			(
				Regex::new(r"^i(?P<index>\d+)$").unwrap(),
				"https://example.com/$index".to_string(),
			),
		];
		let uri_mappings = UriMappings::new(HashMap::new(), pattern);

		// Pattern is close, but does not match
		assert!(uri_mappings.match_pattern("i12.12").is_err());
		assert!(uri_mappings.match_pattern("i-1212").is_err());
		assert!(uri_mappings.match_pattern("i1212g").is_err());
		assert!(uri_mappings.match_pattern("-i1212g").is_err());

		// Pattern matches, but not URI
		assert!(uri_mappings.match_pattern("Solo, Jaina").is_err());

		// Pattern matches and is URI
		let result = uri_mappings.match_pattern("i1212");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/1212")?);

		Ok(())
	}

	#[test]
	fn redirect_standard_and_pattern_uris() -> Result<(), InvalidUri> {
		let standard = HashMap::from([
			("i".to_string(), Uri::from_str("https://example.com")?),
			("i5".to_string(), Uri::from_str("https://example.com/five")?),
			(
				"unrelated".to_string(),
				Uri::from_str("https://example.com/byebye")?,
			),
		]);
		let pattern = vec![
			(
				Regex::new(r"^(?P<index>\d+)$").unwrap(),
				"https://example.com/$index".to_string(),
			),
			(
				Regex::new(r"^i(?P<index>\d+)$").unwrap(),
				"https://example.com/$index".to_string(),
			),
		];
		let uri_mappings = UriMappings::new(standard, pattern);

		// No match at all
		assert!(uri_mappings.match_anything("ithree").is_err());
		assert!(uri_mappings.match_anything("bad").is_err());

		// Standard matches are preferred over pattern matches
		let result = uri_mappings.match_anything("i5");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/five")?);

		// Pattern match used when no standard
		let result = uri_mappings.match_anything("i42");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/42")?);

		Ok(())
	}
}
