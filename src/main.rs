#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]

use dotenv::dotenv;
use regex::Regex;
use std::{
	collections::HashMap, convert::Infallible, env, ffi::OsString, net::SocketAddr, str::FromStr,
};
use substring::Substring;
use warp::{http::Uri, hyper::StatusCode, redirect, reject, Filter, Rejection};

const STANDARD_URI_ENV_NAME: &str = "URSHORT_STANDARD_URI_";
const PATTERN_URI_ENV_NAME: &str = "URSHORT_PATTERN_URI_";
const PATTERN_REGEX_ENV_NAME: &str = "URSHORT_PATTERN_REGEX_";

#[derive(Clone)]
struct UriMappings {
	standard: HashMap<String, Uri>,
	pattern: Vec<(Regex, String)>,
}

impl UriMappings {
	pub fn new() -> UriMappings {
		UriMappings {
			standard: HashMap::new(),
			pattern: Vec::new(),
		}
	}

	pub fn match_standard(&self, uri: &str) -> Result<Uri, &str> {
		match self.standard.get(uri) {
			Some(x) => Ok(x.clone()),
			None => Err("No standard found"),
		}
	}

	pub fn match_pattern(&self, uri: &str) -> Result<Uri, &str> {
		for (regex, uri_pattern) in &self.pattern {
			if !regex.is_match(uri) {
				continue;
			}

			let replacement = regex.replace(uri, uri_pattern);

			return match Uri::from_str(&replacement) {
				Ok(new_uri) => Ok(new_uri),
				Err(_) => Err("Pattern did not create URI"),
			};
		}

		Err("No pattern found")
	}

	pub fn match_anything(&self, uri: &str) -> Result<Uri, &str> {
		match self.match_standard(uri) {
			Ok(standard) => Ok(standard),
			Err(_) => self.match_pattern(uri),
		}
	}
}

#[tokio::main]
async fn main() {
	match dotenv() {
		Ok(_) => println!("Found '.env' file."),
		Err(_) => println!("No '.env' file found."),
	}

	let mut uris = UriMappings::new();
	uris.standard = extract_standard_uris(env::vars_os(), STANDARD_URI_ENV_NAME);
	uris.pattern =
		extract_pattern_uris(env::vars_os(), PATTERN_URI_ENV_NAME, PATTERN_REGEX_ENV_NAME);

	println!("Loaded Standard URIs");
	for (key, uri) in &uris.standard {
		println!("{} {}", key, uri);
	}

	println!("Loaded Pattern URIs");
	for (key, uri) in &uris.pattern {
		println!("{} {}", key, uri);
	}

	// `Get /` Load the root message to inform this is live
	let root_message = warp::path::end().and(warp::get()).and_then(get_root);

	// `Get /:path` Attempt to redirect to a URI
	let short_uri = warp::get()
		.and(warp::path::param::<String>())
		.and_then(move |name: String| get_match(name, uris.clone()));

	let routes = root_message.or(short_uri).recover(error_message);

	let address: SocketAddr = ([127, 0, 0, 1], 3000).into(); // TODO: Address and port should be env vars
	warp::serve(routes).run(address).await;
	println!("Listening on http://{}", address);
}

async fn get_root() -> Result<impl warp::Reply, Infallible> {
	Ok("URShort is running!") // TODO: Project name from cargo? or const. Link to website?
}

async fn get_match(path: String, uris: UriMappings) -> Result<impl warp::Reply, warp::Rejection> {
	match uris.match_anything(&path) {
		Ok(x) => Ok(redirect(x)),
		Err(_) => Err(reject::not_found()),
	}
}

async fn error_message(err: Rejection) -> Result<impl warp::Reply, Infallible> {
	let code;
	let message;

	if err.is_not_found() {
		code = StatusCode::NOT_FOUND;
		message = "URI mapping not found :-(";
	} else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
		code = StatusCode::METHOD_NOT_ALLOWED;
		message = "HTTP Method not supported";
	} else {
		eprintln!("Server error: {:?}", err);
		code = StatusCode::INTERNAL_SERVER_ERROR;
		message = "Whoops, something unexpected went wrong";
	}

	Ok(warp::reply::with_status(message, code))
}

fn extract_standard_uris<I>(env_vars: I, env_var_prefix: &str) -> HashMap<String, Uri>
where
	I: IntoIterator<Item = (OsString, OsString)>,
{
	env_vars
		.into_iter()
		.map(
			|(os_x, os_y)| match (os_x.into_string(), os_y.into_string()) {
				(Ok(x), Ok(y)) => match Uri::from_str(&y) {
					Ok(y) => Ok((x, y)),
					_ => Err("Standard URI not valid"),
				},
				_ => Err("Not valid standard string"),
			},
		)
		.filter(|item| match item {
			Ok((x, _)) => x.starts_with(env_var_prefix),
			_ => false,
		})
		.map(|item| match item {
			Ok((x, y)) => (x.substring(env_var_prefix.len(), x.len()).to_owned(), y),
			_ => unreachable!(),
		})
		.collect()
}

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
		.fold(vec![String::new();uri_length], |mut list: Vec<String>, (x, y)| {
			list[x] = y;
			list
		});

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
		.fold(vec![Regex::new("").unwrap();regex_length], |mut list: Vec<Regex>, (x, y)| {
			list[x] = y;
			list
		});

	regex_list
		.into_iter()
		.zip(uri_list)
		.collect::<Vec<(Regex, String)>>()
}

#[cfg(test)]
mod tests {
	#![allow(clippy::unnecessary_wraps)]

	use std::{ffi::OsString, str::FromStr};
	use warp::http::uri::InvalidUri;

	use super::*;

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

		let result =
			extract_standard_uris(variables_from_environment.into_iter(), STANDARD_URI_ENV_NAME);

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
		let mut uris = UriMappings::new();
		uris.standard = HashMap::from([
			("test".to_string(), Uri::from_str("https://example.com")?),
			("1/1".to_string(), Uri::from_str("https://example.com/1")?),
			("3.14".to_string(), Uri::from_str("https://example.com/pi")?),
		]);

		// No matches
		assert!(uris.match_standard("/invalid").is_err());

		// Can't match an invalid URI, because it must be a URI to be loaded into the hashmap

		// Standard matches
		assert_eq!(
			uris.match_standard("test").unwrap(),
			Uri::from_str("https://example.com")?
		);
		assert_eq!(
			uris.match_standard("1/1").unwrap(),
			Uri::from_str("https://example.com/1")?
		);
		assert_eq!(
			uris.match_standard("3.14").unwrap(),
			Uri::from_str("https://example.com/pi")?
		);

		Ok(())
	}

	#[test]
	fn redirect_pattern_uris() -> Result<(), InvalidUri> {
		let mut uris = UriMappings::new();
		uris.pattern = vec![
			(
				Regex::new(r"(?P<last>[^,\s]+),\s+(?P<first>\S+)").unwrap(),
				"$first $last".to_string(),
			),
			(
				Regex::new(r"^i(?P<index>\d+)$").unwrap(),
				"https://example.com/$index".to_string(),
			),
		];

		// Pattern is close, but does not match
		assert!(uris.match_pattern("i12.12").is_err());
		assert!(uris.match_pattern("i-1212").is_err());
		assert!(uris.match_pattern("i1212g").is_err());
		assert!(uris.match_pattern("-i1212g").is_err());

		// Pattern matches, but not URI
		assert!(uris.match_pattern("Solo, Jaina").is_err());

		// Pattern matches and is URI
		let result = uris.match_pattern("i1212");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/1212")?);

		Ok(())
	}

	#[test]
	fn redirect_standard_and_pattern_uris() -> Result<(), InvalidUri> {
		let mut uris = UriMappings::new();
		uris.standard = HashMap::from([
			("i".to_string(), Uri::from_str("https://example.com")?),
			("i5".to_string(), Uri::from_str("https://example.com/five")?),
			(
				"unrelated".to_string(),
				Uri::from_str("https://example.com/byebye")?,
			),
		]);
		uris.pattern = vec![
			(
				Regex::new(r"^(?P<index>\d+)$").unwrap(),
				"https://example.com/$index".to_string(),
			),
			(
				Regex::new(r"^i(?P<index>\d+)$").unwrap(),
				"https://example.com/$index".to_string(),
			),
		];
		// No match at all
		assert!(uris.match_anything("ithree").is_err());
		assert!(uris.match_anything("bad").is_err());

		// Standard matches are preferred over pattern matches
		let result = uris.match_anything("i5");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/five")?);

		// Pattern match used when no standard
		let result = uris.match_anything("i42");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/42")?);

		Ok(())
	}
}
