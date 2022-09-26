use std::{collections::HashMap, ffi::OsString, str::FromStr};

use axum::http::Uri;
use regex::Regex;
use substring::Substring;

/// Extract the configured port number, if one is there, from the environmental variables
pub fn extract_port_number<I>(env_vars: I, env_var_prefix: &str) -> Option<u16>
where
	I: IntoIterator<Item = (OsString, OsString)>,
{
	env_vars
		.into_iter()
		.find_map(|(x, y)| match (x.into_string(), y.into_string()) {
			(Ok(x), Ok(y)) if x.eq(env_var_prefix) => {
				if let Ok(y) = y.parse::<u16>() {
					return Some(y);
				}
				None
			}
			_ => None,
		})
}

/// Extract all available standard URIs from the environmental variables
pub fn extract_standard_uris<I>(env_vars: I, env_var_prefix: &str) -> HashMap<String, Uri>
where
	I: IntoIterator<Item = (OsString, OsString)>,
{
	env_vars
		.into_iter()
		.filter_map(|(x, y)| match (x.into_string(), y.into_string()) {
			(Ok(x), Ok(y)) if x.starts_with(env_var_prefix) => match Uri::from_str(&y) {
				Ok(y) => {
					let x = x.substring(env_var_prefix.len(), x.len()).to_owned();
					Some((x, y))
				}
				_ => None,
			},
			_ => None,
		})
		.collect()
}

/// Extract all available pattern URIs from the environmental variables
pub fn extract_pattern_uris<I>(
	env_vars: I,
	env_var_uri_prefix: &str,
	env_var_regex_prefix: &str,
) -> Vec<(Regex, String)>
where
	I: IntoIterator<Item = (OsString, OsString)>,
{
	// Partition is used, because env_vars needs to be split into multiple collections since it's consumed upon iteration
	let (uri_list, everything_else): (Vec<_>, Vec<_>) = env_vars.into_iter().partition(
		|(x, _)| matches!(x.clone().into_string(), Ok(x) if x.starts_with(env_var_uri_prefix)),
	);

	let (regex_list, _): (Vec<_>, _) = everything_else.into_iter().partition(
		|(x, _)| matches!(x.clone().into_string(), Ok(x) if x.starts_with(env_var_regex_prefix)),
	);

	let uri_length = uri_list.len();
	let uri_list = uri_list
		.into_iter()
		.filter_map(|(x, y)| match (x.into_string(), y.into_string()) {
			(Ok(x), Ok(y)) => match x[env_var_uri_prefix.len()..].parse::<usize>() {
				Ok(x) => Some((x, y)),
				_ => None,
			},
			_ => None,
		})
		.fold(
			vec![String::new(); uri_length],
			|mut list: Vec<String>, (x, y)| {
				list[x] = y;
				list
			},
		);

	let regex_length = regex_list.len();
	let regex_list = regex_list
		.into_iter()
		.filter_map(|(x, y)| match (x.into_string(), y.into_string()) {
			(Ok(x), Ok(y)) => {
				match (
					x[env_var_regex_prefix.len()..].parse::<usize>(),
					Regex::from_str(&y),
				) {
					(Ok(x), Ok(y)) => Some((x, y)),
					_ => None,
				}
			}
			_ => None,
		})
		.fold(
			vec![Regex::new("").unwrap(); regex_length],
			|mut list: Vec<Regex>, (x, y)| {
				list[x] = y;
				list
			},
		);

	regex_list
		.into_iter()
		.zip(uri_list)
		.collect::<Vec<(Regex, String)>>()
}

mod tests {
	#![allow(clippy::unnecessary_wraps)]

	// Unclear why this is treated as an unused import, but this patches the problem
	#[allow(unused_imports)]
	use super::*;

	#[test]
	fn load_port_env_var() -> Result<(), ()> {
		const PORT_ENV_NAME: &str = "TEST_PORT_ENV_NAME";
		let port_to_pass = 8080;

		let unrelated_key = "test";
		let unrelated_value = "test";
		let not_number = "notANumber";
		let signed_number = "-3000";
		let valid_value = port_to_pass.to_string();
		let not_the_first_valid_value = "8000";

		let variables_from_environment = vec![
			(
				OsString::from_str(unrelated_key).unwrap(),
				OsString::from_str(unrelated_value).unwrap(),
			),
			(
				OsString::from_str(PORT_ENV_NAME).unwrap(),
				OsString::from_str(not_number).unwrap(),
			),
			(
				OsString::from_str(PORT_ENV_NAME).unwrap(),
				OsString::from_str(signed_number).unwrap(),
			),
			(
				OsString::from_str(PORT_ENV_NAME).unwrap(),
				OsString::from_str(valid_value.as_str()).unwrap(),
			),
			(
				OsString::from_str(PORT_ENV_NAME).unwrap(),
				OsString::from_str(not_the_first_valid_value).unwrap(),
			),
		];

		let result = extract_port_number(variables_from_environment.into_iter(), PORT_ENV_NAME);

		assert_eq!(result, Some(port_to_pass));

		Ok(())
	}

	#[test]
	fn load_standard_env_var() -> Result<(), ()> {
		const STANDARD_URI_ENV_NAME: &str = "TEST_STANDARD_URI_ENV_NAME";

		let simple_key = "test";
		let simple_value = "https://example.com/";
		let unused_key = "unused";
		let unused_value = "https://example.com/unused";
		let empty_key = "";
		let empty_value = "https://example.com/empty";
		let overridden_duplicate_key = "override";
		let overridden_duplicate_value = "https://example.com/overridden";
		let override_duplicate_key = "override";
		let override_duplicate_value = "https://example.com/override";

		let variables_from_environment = vec![
			(
				OsString::from_str(format!("{}{}", STANDARD_URI_ENV_NAME, simple_key).as_str())
					.unwrap(),
				OsString::from_str(simple_value).unwrap(),
			),
			(
				OsString::from_str(unused_key).unwrap(),
				OsString::from_str(unused_value).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", STANDARD_URI_ENV_NAME, empty_key).as_str())
					.unwrap(),
				OsString::from_str(empty_value).unwrap(),
			),
			(
				OsString::from_str(
					format!("{}{}", STANDARD_URI_ENV_NAME, overridden_duplicate_key).as_str(),
				)
				.unwrap(),
				OsString::from_str(overridden_duplicate_value).unwrap(),
			),
			(
				OsString::from_str(
					format!("{}{}", STANDARD_URI_ENV_NAME, override_duplicate_key).as_str(),
				)
				.unwrap(),
				OsString::from_str(override_duplicate_value).unwrap(),
			),
		];

		let result = extract_standard_uris(
			variables_from_environment.into_iter(),
			STANDARD_URI_ENV_NAME,
		);

		assert_eq!(
			result.get(simple_key).unwrap(),
			&Uri::from_str(simple_value).unwrap()
		);
		assert!(result.get(unused_key).is_none());
		assert_eq!(
			result.get(empty_key).unwrap(),
			&Uri::from_str(empty_value).unwrap()
		);
		assert_eq!(
			result.get(override_duplicate_key).unwrap(),
			&Uri::from_str(override_duplicate_value).unwrap()
		);

		Ok(())
	}

	#[test]
	fn load_pattern_env_var() -> Result<(), ()> {
		const PATTERN_URI_ENV_NAME: &str = "TEST_PATTERN_URI_ENV_NAME";
		const PATTERN_REGEX_ENV_NAME: &str = "TEST_PATTERN_REGEX_ENV_NAME";

		let regex_0 = "a*";
		let value_0 = "https://example.com/";
		let regex_1 = r"^i(a+)$";
		let value_1 = "https://example.com/a";
		let regex_2 = r"^i(d+)$";
		let value_2 = "https://example.com/$1";
		let regex_3 = r"^i(?P<index>\d+)$";
		let value_3 = "https://example.com/$index";

		let variables_from_environment = vec![
			(
				OsString::from_str(format!("{}{}", PATTERN_REGEX_ENV_NAME, 1).as_str()).unwrap(),
				OsString::from_str(regex_1).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_REGEX_ENV_NAME, 0).as_str()).unwrap(),
				OsString::from_str(regex_0).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_URI_ENV_NAME, 0).as_str()).unwrap(),
				OsString::from_str(value_0).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_URI_ENV_NAME, 1).as_str()).unwrap(),
				OsString::from_str(value_1).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_REGEX_ENV_NAME, 2).as_str()).unwrap(),
				OsString::from_str(regex_2).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_URI_ENV_NAME, 2).as_str()).unwrap(),
				OsString::from_str(value_2).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_REGEX_ENV_NAME, 3).as_str()).unwrap(),
				OsString::from_str(regex_3).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_URI_ENV_NAME, 3).as_str()).unwrap(),
				OsString::from_str(value_3).unwrap(),
			),
		];

		let result = extract_pattern_uris(
			variables_from_environment,
			PATTERN_URI_ENV_NAME,
			PATTERN_REGEX_ENV_NAME,
		);

		assert_eq!(result[0].0.to_string(), regex_0);
		assert_eq!(result[0].1, value_0);

		// Testing that patterns can be added in any order
		assert_eq!(result[1].0.to_string(), regex_1);
		assert_eq!(result[1].1, value_1);

		assert_eq!(result[2].0.to_string(), regex_2);
		assert_eq!(result[2].1, value_2);

		assert_eq!(result[3].0.to_string(), regex_3);
		assert_eq!(result[3].1, value_3);

		Ok(())
	}
}
