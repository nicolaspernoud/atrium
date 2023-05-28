# atrium

Atrium is a web server, reverse proxy and webdav server with user authentication.  
It comes with a multiplatform client application.

Rust/Flutter version of Vestibule.

## Installation

Example docker installation can be found [HERE](https://github.com/nicolaspernoud/atrium/blob/main/scripts/deploy/up.sh).  
Use your own GeoLite2-City.mmdb or remove the line.

Example configuration can be found [HERE](https://github.com/nicolaspernoud/atrium/blob/main/backend/atrium.yaml).  
To start quickly : configure your hostname, and set `tls_mode` to `Auto`.

## Configuration

See [atrium.yaml](https://github.com/nicolaspernoud/atrium/blob/main/backend/atrium.yaml) for configuration options and examples.

The `hostname` configuration can be overridden with the environment variable `MAIN_HOSTNAME`.

## DNS

Your DNS configuration should be as below :
|Domain|Type|Target|
|--|--|--|
|your.hostname|A|Your machine IPv4|
|your.hostname|AAAA|Your machine IPv6|
|\*.your.hostname|CNAME|your.hostname|

## Development

### Update main from development and set development to follow main

```bash
git checkout main
git merge development --squash
# Alter commit message and commit
git checkout development
git reset --hard main
git push --force
```

### Clean useless dependencies and features

```bash
cargo install cargo-udeps --locked
cargo +nightly udeps --all-targets
cargo install cargo-unused-features
unused-features analyze --bins --lib --tests && unused-features build-report --input "report.json" && unused-features prune --input "report.json"
```

### Run flutter debug server to allow clipboard access

```
flutter run -d chrome --web-hostname atrium.127.0.0.1.nip.io --web-port 3000 --web-browser-flag "--unsafely-treat-insecure-origin-as-secure=http://atrium.127.0.0.1.nip.io:3000"
```
