# URShort

[![Latest Docker Image Version][Docker Image]][Docker Download]
[![Latest Crates.io Version][Crates Image]][Crates Download]
[![Latest Crates.io Version][Build Image]][Build]
[![MIT license][License Image]][License]

[Docker Image]: https://img.shields.io/docker/v/mirdaki/urshort?style=flat-square
[Docker Download]: https://hub.docker.com/r/mirdaki/urshort
[Crates Image]: https://img.shields.io/crates/v/urshort?style=flat-square
[Crates Download]: https://crates.io/crates/urshort
[Build Image]: https://img.shields.io/github/workflow/status/mirdaki/urshort/Rust%20Check/main?style=flat-square
[Build]: https://github.com/mirdaki/urshort/actions/workflows/rust-check.yml
[License Image]: https://img.shields.io/crates/l/urshort?style=flat-square
[License]: LICENSE.md

A blazingly fast and amazingly simple URL shortener designed for self-hosters.

Support regular vanity URI mappings as well as (Perl style) regex based mapping!

It uses environmental variables to configure all matches and loads them into an in-memory model for near instant access! Designed for hosting with containers and for individual use.

## Build

Use the standard [Rust tools](https://www.rust-lang.org/tools/install). There is also a [Dev Container](https://code.visualstudio.com/docs/remote/create-dev-container) if you would prefer to run it that way.

```bash
cargo build

# Also recommended for linting suggestions
cargo clippy

# And for consistent formatting
cargo fmt
```

For creating the Docker container. Use the included [Dockerfile](Dockerfile) and this:

```bash
docker build -t urshort:latest .
```

## Installation

It is recommended to use [Docker](https://www.docker.com/) to use URShort:

```bash
docker pull mirdaki/urshort
```

If you have prefer, you can also install it via [cargo](https://doc.rust-lang.org/cargo/):

```bash
cargo install urshort
```

Or download directly from the [releases](https://github.com/mirdaki/urshort/releases).

## Configuration

As all configuration is stored in environmental variables, it is recommenced to store them in an `.env` file for ease of tracking and updating. For example:

```.env
URSHORT_STANDARD_URI_test=https://example.com/
URSHORT_STANDARD_URI_test2=https://example.com/2

URSHORT_PATTERN_REGEX_0='^i(?P<index>\d+)$'
URSHORT_PATTERN_URI_0='https://example.com/$index'
URSHORT_PATTERN_REGEX_1='^(\d+)$'
URSHORT_PATTERN_URI_1='https://example.com/$1'
```

### Standard Mapping

A standard vanity mapping would do something like: `hello -> example.com/hello`

Using this config:
- Path: `hello`
- Redirect: `https://example.com/hello`

This is stored in a single environmental variable:

```bash
# <> is used to indicate the values to be changes
URSHORT_STANDARD_URI_<path>=<redirect>

# Actual example
URSHORT_STANDARD_URI_test=https://example.com/
```

Standard mappings will override any regex mapping.

### Regex Mapping

A single regex pattern mapping could do something like:
```
i1 -> example.com/1
i42 -> example.com/42
i9001 -> example.com/9001
...
```

Using this config:
- Regex pattern: `'^i(?P<index>\d+)$'`
- Pattern redirect `'https://example.com/$index'`

This is stored in two single environmental variables. The first contains
```bash
# <> is used to indicate the values to be changes
# The first environmental variable determines the place in which the pattern is evaluated and the regex for that pattern
URSHORT_PATTERN_REGEX_<place>='<regex>'
# The second environmental variable must match the place of the first and contains the pattern redirect
URSHORT_PATTERN_URI_<place>='<redirect>'

# Actual example
URSHORT_PATTERN_REGEX_0='^i(?P<index>\d+)$'
URSHORT_PATTERN_URI_0='https://example.com/$index'
```

Be sure to quote the values. Be careful with the order you have the mappings.

### Port

You can specify a port the service will use. If not give, the default of `54027` will be used.

This is useful when running the service directly. If you use Docker, it is recommended to use the built in port mapping features instead of this.

Example environmental variable: `URSHORT_PORT=7777`

## Usage

Please use a web server, such as [Nginx](https://nginx.org/en/) or [Traefik](https://traefik.io/) in front of URShort.

[Docker compose](https://github.com/docker/compose) is the recommended way to run URShort. An example file is [here](docker-compose.yaml). Load that file with your `.env` file and run:

```bash
docker-compose up
```

You can also use the Docker run command directly:

```bash
docker run -d \
  --name=urshort \
  -p 54027:54027 \
  -e URSHORT_STANDARD_URI_test=https://example.com/ \
  -e URSHORT_PATTERN_REGEX_0='^i(?P<index>\d+)$' \
  -e URSHORT_PATTERN_URI_0='https://example.com/$index' \
  --restart unless-stopped \
  mirdaki/urshort:latest
```

Or if you have the bare executable, run `urshort` at the location of you `.env` file (or after your configuration is loaded directly into the environment).

## Built With

Thank you to all the projects that helped make this possible!

- [Rust](https://www.rust-lang.org/) for being an awesome language to build with
- [Axum](https://github.com/tokio-rs/axum) for doing the whole URL redirect thing

## Contributing

Please read [CONTRIBUTING.md](CONTRIBUTING.md) for how to contribute to the project.

## License

This project is licensed under the MIT License - see the [LICENSE.md](LICENSE.md) file for details.
