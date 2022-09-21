#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]

use axum::{
	extract::Path,
	http::Uri,
	response::{Html, Redirect},
	routing::get,
	Router,
};
use dotenv::dotenv;
use regex::Regex;
use std::{
	collections::HashMap,
	env,
	ffi::OsString,
	fs::File,
	io::{Error, Read},
	net::SocketAddr,
	str::FromStr,
	sync::Arc,
};
use substring::Substring;

const STANDARD_URI_ENV_NAME: &str = "URSHORT_STANDARD_URI_";
const PATTERN_URI_ENV_NAME: &str = "URSHORT_PATTERN_URI_";
const PATTERN_REGEX_ENV_NAME: &str = "URSHORT_PATTERN_REGEX_";
const PORT_ENV_NAME: &str = "URSHORT_PORT";
const DEFAULT_PORT: u16 = 3000;

/// Contains the mapping of URIs to redirect to
struct UriMappings {
	standard: HashMap<String, Uri>,
	pattern: Vec<(Regex, String)>,
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

#[tokio::main]
async fn main() {
	match dotenv() {
		Ok(_) => println!("Found '.env' file."),
		Err(_) => println!("No '.env' file found."),
	}
	println!();

	let standard_uris = extract_standard_uris(env::vars_os(), STANDARD_URI_ENV_NAME);
	let pattern_uris =
		extract_pattern_uris(env::vars_os(), PATTERN_URI_ENV_NAME, PATTERN_REGEX_ENV_NAME);
	let uri_mappings = Arc::new(UriMappings::new(standard_uris, pattern_uris));

	let port: u16 = extract_port_number(env::vars_os(), PORT_ENV_NAME).unwrap_or(DEFAULT_PORT);

	println!("Loaded Standard URIs:");
	for (key, uri) in &uri_mappings.standard {
		println!("{} {}", key, uri);
	}
	println!();

	println!("Loaded Pattern URIs:");
	for (key, uri) in &uri_mappings.pattern {
		println!("{} {}", key, uri);
	}
	println!();

	// Load welcome page
	let welcome_page = Arc::new(load_html_page("./src/index.html").unwrap_or_else(|error| {
		eprint!("{}", error);
		"Home page failed to load".to_string()
	}));

	// Load error page
	let error_page = Arc::new(load_html_page("./src/error.html").unwrap_or_else(|error| {
		eprint!("{}", error);
		"Error page failed to load".to_string()
	}));

	// build our application with a route
	let app = Router::new()
		// `GET /` for homepage
		.route("/", get(move || get_root(welcome_page.clone())))
		// `POST /:parameter` for vanity URL or error page if it fails
		.route(
			"/:parameter",
			get(move |Path(parameter): Path<String>| {
				get_match_and_redirect(parameter, uri_mappings.clone(), error_page.clone())
			}),
		);

	let address = SocketAddr::from(([127, 0, 0, 1], port));
	println!("Listening on http://{}", address);

	axum::Server::bind(&address)
		.serve(app.into_make_service())
		.await
		.unwrap();
}

/// Load a local file into a string
fn load_html_page(path: &str) -> Result<String, Error> {
	let mut file = File::open(path)?;
	let mut page = String::new();
	file.read_to_string(&mut page)?;
	Ok(page)
}

/// Return the welcome screen, indicating it is running
async fn get_root(welcome_page: Arc<String>) -> Html<String> {
	Html(welcome_page.to_string())
}

/// Attempts to get a match and redirect if one is found
async fn get_match_and_redirect(
	path: String,
	uri_mappings: Arc<UriMappings>,
	error_page: Arc<String>,
) -> Result<axum::response::Redirect, Html<String>> {
	match uri_mappings.match_anything(&path) {
		Ok(x) => Ok(Redirect::temporary(x.to_string().as_str())),
		Err(_) => Err(Html(error_page.to_string())),
	}
}

/// Extract the configured port number, if one is there, from the environmental variables
fn extract_port_number<I>(env_vars: I, env_var_prefix: &str) -> Option<u16>
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
fn extract_standard_uris<I>(env_vars: I, env_var_prefix: &str) -> HashMap<String, Uri>
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
fn extract_pattern_uris<I>(
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

#[cfg(test)]
mod tests {
	#![allow(clippy::unnecessary_wraps)]

	use axum::http::uri::InvalidUri;
	use std::{ffi::OsString, str::FromStr};

	use super::*;

	#[test]
	fn load_port_env_var() -> Result<(), ()> {
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
