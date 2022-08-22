use dotenv::dotenv;
use regex::Regex;
use std::{
	collections::HashMap, convert::Infallible, env, ffi::OsString, net::SocketAddr,
	str::FromStr,
};
use substring::Substring;
use warp::{http::Uri, hyper::StatusCode, redirect, reject, Filter, Rejection};

const VANITY_URL_ENV_NAME: &str = "URSHORT_VANITY_URL_";
const PATTERN_URL_ENV_NAME: &str = "URSHORT_PATTERN_URL_";
const PATTERN_REGEX_ENV_NAME: &str = "URSHORT_PATTERN_REGEX_";

#[derive(Clone)]
struct UrlMappings {
	vanity: HashMap<String, Uri>,
	pattern: Vec<(Regex, String)>,
}

impl UrlMappings {
	pub fn new() -> UrlMappings {
		UrlMappings {
			vanity: HashMap::new(),
			pattern: Vec::new(),
		}
	}

	pub fn match_vanity(&self, uri: &str) -> Result<Uri, &str> {
		match self.vanity.get(uri) {
			Some(x) => Ok(x.clone()),
			None => Err("No vanity found"),
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
		match self.match_vanity(uri) {
			Ok(vanity) => Ok(vanity),
			Err(_) => self.match_pattern(uri),
		}
	}
}

#[tokio::main]
async fn main() {
	match dotenv() {
		Ok(_) => println!("Loading values from '.env' file."),
		Err(_) => println!("No '.env' file found."),
	}

	let mut urls = UrlMappings::new();
	urls.vanity = extract_vanity_urls(env::vars_os(), VANITY_URL_ENV_NAME);
	urls.pattern =
		extract_pattern_urls(env::vars_os(), PATTERN_URL_ENV_NAME, PATTERN_REGEX_ENV_NAME);

	println!("Vanity URLs");
	for (key, url) in &urls.vanity {
		println!("{} {}", key, url);
	}

	println!("Pattern URLs");
	for (key, url) in &urls.pattern {
		println!("{} {}", key, url);
	}

	// `Get /` Load the root message to inform this is live
	let root_message = warp::path::end().and(warp::get()).and_then(get_root);

	// `Get /:path` Attempt to redirect to a URL
	let short_url = warp::get()
		.and(warp::path::param::<String>())
		.and_then(move |name: String| get_match(name, urls.clone()));

	let routes = root_message.or(short_url).recover(error_message);

	let address: SocketAddr = ([127, 0, 0, 1], 3000).into(); // TODO: Address and port should be env vars
	warp::serve(routes).run(address).await;
	println!("Listening on http://{}", address);
}

async fn get_root() -> Result<impl warp::Reply, Infallible> {
	Ok("URShort is running!") // TODO: Project name from cargo? or const
}

async fn get_match(path: String, urls: UrlMappings) -> Result<impl warp::Reply, warp::Rejection> {
	match urls.match_anything(&path) {
		Ok(x) => Ok(redirect(x)),
		Err(_) => Err(reject::not_found()),
	}
}

async fn error_message(err: Rejection) -> Result<impl warp::Reply, Infallible> {
	let code;
	let message;

	if err.is_not_found() {
		code = StatusCode::NOT_FOUND;
		message = "URL mapping not found :-(";
	} else if err.find::<warp::reject::MethodNotAllowed>().is_some() {
		code = StatusCode::METHOD_NOT_ALLOWED;
		message = "Method not supported";
	} else {
		eprintln!("Server error: {:?}", err); // TODO: Should log or print errors?
		code = StatusCode::INTERNAL_SERVER_ERROR;
		message = "Whoops, something unexpected went wrong";
	}

	Ok(warp::reply::with_status(message, code))
}

fn extract_vanity_urls<I>(env_vars: I, env_var_prefix: &str) -> HashMap<String, Uri>
where
	I: IntoIterator<Item = (OsString, OsString)>,
{
	env_vars
		.into_iter()
		.map(
			|(os_x, os_y)| match (os_x.into_string(), os_y.into_string()) {
				(Ok(x), Ok(y)) => match Uri::from_str(&y) {
					Ok(y) => Ok((x, y)),
					_ => Err("URI not valid"),
				},
				_ => Err("Not valid string"),
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

fn extract_pattern_urls<I>(
	env_vars: I,
	env_var_url_prefix: &str,
	env_var_regex_prefix: &str,
) -> Vec<(Regex, String)>
where
	I: IntoIterator<Item = (OsString, OsString)>,
{
	// Partition is used, because env_vars needs to be split into multiple collections since it's consumed upon iteration
	let (url_list, everything_else): (Vec<_>, Vec<_>) = env_vars.into_iter().partition(
		|(x, _)| matches!(x.to_owned().into_string(), Ok(x) if x.starts_with(env_var_url_prefix)),
	);

	let (regex_list, _): (Vec<_>, _) = everything_else.into_iter().partition(
		|(x, _)| matches!(x.to_owned().into_string(), Ok(x) if x.starts_with(env_var_regex_prefix)),
	);

	let url_list = url_list
		.into_iter()
		.filter_map(|(x, y)| match (x.into_string(), y.into_string()) {
			(Ok(x), Ok(y)) => match x[env_var_url_prefix.len()..].parse::<usize>() {
				Ok(x) => Some((x, y)),
				_ => None,
			},
			_ => None,
		})
		.fold(Vec::<String>::new(), |mut list: Vec<String>, (x, y)| {
			list.insert(x, y);
			list
		});

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
		.fold(Vec::<Regex>::new(), |mut list: Vec<Regex>, (x, y)| {
			list.insert(x, y);
			list
		});

	regex_list
		.into_iter()
		.zip(url_list)
		.collect::<Vec<(Regex, String)>>()
}
#[cfg(test)]
mod tests {
	use std::{ffi::OsString, str::FromStr};
	use warp::http::uri::InvalidUri;

	use super::*;

	#[test]
	fn load_vanity_env_var() -> Result<(), ()> {
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
				OsString::from_str(format!("{}{}", VANITY_URL_ENV_NAME, simple_key).as_str())
					.unwrap(),
				OsString::from_str(simple_value).unwrap(),
			),
			(
				OsString::from_str(unused_key).unwrap(),
				OsString::from_str(unused_value).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", VANITY_URL_ENV_NAME, empty_key).as_str())
					.unwrap(),
				OsString::from_str(empty_value).unwrap(),
			),
			(
				OsString::from_str(
					format!("{}{}", VANITY_URL_ENV_NAME, overridden_duplicate_key).as_str(),
				)
				.unwrap(),
				OsString::from_str(overridden_duplicate_value).unwrap(),
			),
			(
				OsString::from_str(
					format!("{}{}", VANITY_URL_ENV_NAME, override_duplicate_key).as_str(),
				)
				.unwrap(),
				OsString::from_str(override_duplicate_value).unwrap(),
			),
		];

		let result =
			extract_vanity_urls(variables_from_environment.into_iter(), VANITY_URL_ENV_NAME);

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
		let simple_regex = "a*";
		let simple_value = "https://example.com/";

		let variables_from_environment = vec![
			(
				OsString::from_str(format!("{}{}", PATTERN_REGEX_ENV_NAME, 0).as_str()).unwrap(),
				OsString::from_str(simple_regex).unwrap(),
			),
			(
				OsString::from_str(format!("{}{}", PATTERN_URL_ENV_NAME, 0).as_str()).unwrap(),
				OsString::from_str(simple_value).unwrap(),
			),
		];

		let result = extract_pattern_urls(
			variables_from_environment,
			PATTERN_URL_ENV_NAME,
			PATTERN_REGEX_ENV_NAME,
		);

		// TODO: Test a couple more regexs and verify invalid ones are not loaded (check if the order of regexes being loaded matters)

		assert_eq!(result[0].0.to_string(), simple_regex);
		assert_eq!(result[0].1, simple_value);

		Ok(())
	}

	#[test]
	fn vanity_urls() -> Result<(), InvalidUri> {
		let mut urls = UrlMappings::new();
		urls.vanity = HashMap::from([
			("test".to_string(), Uri::from_str("https://example.com")?),
			("1/1".to_string(), Uri::from_str("https://example.com/1")?),
			("3.14".to_string(), Uri::from_str("https://example.com/pi")?),
		]);

		// No matches
		assert!(urls.match_vanity("/invalid").is_err());

		// Can't match an invalid URI, because it must be a URI to be loaded into the hashmap

		// Vanity matches
		assert_eq!(
			urls.match_vanity("test").unwrap(),
			Uri::from_str("https://example.com")?
		);
		assert_eq!(
			urls.match_vanity("1/1").unwrap(),
			Uri::from_str("https://example.com/1")?
		);
		assert_eq!(
			urls.match_vanity("3.14").unwrap(),
			Uri::from_str("https://example.com/pi")?
		);

		Ok(())
	}

	#[test]
	fn pattern_urls() -> Result<(), InvalidUri> {
		let mut urls = UrlMappings::new();
		urls.pattern = vec![
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
		assert!(urls.match_pattern("i12.12").is_err());
		assert!(urls.match_pattern("i-1212").is_err());
		assert!(urls.match_pattern("i1212g").is_err());
		assert!(urls.match_pattern("-i1212g").is_err());

		// Pattern matches, but not URI
		assert!(urls.match_pattern("Solo, Jaina").is_err());

		// Pattern matches and is URI
		let result = urls.match_pattern("i1212");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/1212")?);

		Ok(())
	}

	#[test]
	fn vanity_and_pattern_urls() -> Result<(), InvalidUri> {
		let mut urls = UrlMappings::new();
		urls.vanity = HashMap::from([
			("i".to_string(), Uri::from_str("https://example.com")?),
			("i5".to_string(), Uri::from_str("https://example.com/five")?),
			(
				"unrelated".to_string(),
				Uri::from_str("https://example.com/byebye")?,
			),
		]);
		urls.pattern = vec![
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
		assert!(urls.match_anything("ithree").is_err());
		assert!(urls.match_anything("bad").is_err());

		// Vanity matches are preferred over pattern matches
		let result = urls.match_anything("i5");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/five")?);

		// Pattern match used when no vanity
		let result = urls.match_anything("i42");
		assert!(result.is_ok());
		assert_eq!(result.unwrap(), Uri::from_str("https://example.com/42")?);

		Ok(())
	}
}
