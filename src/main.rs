#![warn(clippy::pedantic)]
#![allow(clippy::unused_async)]

use axum::{
	extract::Path,
	response::{Html, Redirect},
	routing::get,
	Router,
};
use dotenv::dotenv;

use std::{env, future::Future, net::SocketAddr, sync::Arc};

mod environment;
mod uri_mappings;
use crate::{
	environment::{extract_pattern_uris, extract_port_number, extract_standard_uris},
	uri_mappings::UriMappings,
};

const STANDARD_URI_ENV_NAME: &str = "URSHORT_STANDARD_URI_";
const PATTERN_URI_ENV_NAME: &str = "URSHORT_PATTERN_URI_";
const PATTERN_REGEX_ENV_NAME: &str = "URSHORT_PATTERN_REGEX_";
const PORT_ENV_NAME: &str = "URSHORT_PORT";
const DEFAULT_PORT: u16 = 54027;

#[tokio::main]
async fn main() {
	// Notify user if the .env file was used, but don't if one was not found
	// as it may be confusing if one was used by docker, but not passed locally
	if dotenv().is_ok() {
		println!("Loaded local '.env' file");
	}
	println!();

	// Load the envirmental variables
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

	// Setup REST API
	let app = Router::new()
		// `GET /` for homepage
		.route("/", get(index_page))
		// `POST /:parameter` for vanity URL or error page if it fails
		.route(
			"/:parameter",
			get(move |Path(parameter): Path<String>| {
				get_match_and_redirect(parameter, uri_mappings.clone(), error_page)
			}),
		);

	let address = SocketAddr::from(([0, 0, 0, 0], port));
	println!("Listening on http://{}", address);

	axum::Server::bind(&address)
		.serve(app.into_make_service())
		.await
		.unwrap();
}

/// Load the index.html page at compile time
async fn index_page() -> Html<&'static str> {
	Html(std::include_str!("../assets/index.html"))
}

/// Load the error.html page at compile time
async fn error_page() -> Html<&'static str> {
	Html(std::include_str!("../assets/error.html"))
}

/// Attempts to get a match and redirect if one is found
async fn get_match_and_redirect<F, Fut>(
	path: String,
	uri_mappings: Arc<UriMappings>,
	error_page: F,
) -> Result<axum::response::Redirect, Html<&'static str>>
where
	F: Fn() -> Fut,
	Fut: Future<Output = Html<&'static str>>,
{
	match uri_mappings.match_anything(&path) {
		Ok(x) => Ok(Redirect::temporary(x.to_string().as_str())),
		Err(_) => Err(error_page().await),
	}
}
