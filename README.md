# atrium

Atrium is a web server, reverse proxy and webdav server with user authentication.  
It comes with a multiplatform client application.

Rust/Flutter version of Vestibule.

## ⚠️ Breaking change ⚠️

From version 1.8.0 atrium encrypted file format changed to ensure future crypto agility.
To migrate from 1.7.x to 1.8+ use the `backend/src/bin/convert_encryption.rs` binary to convert encrypted files to the new format.
The encryption key is not needed since it is only an encryption type prefix added to every files.

## Installation

Example service installation can be found [HERE](https://github.com/nicolaspernoud/atrium/blob/main/scripts/systemd/install_atrium.sh).

**OR**

Example docker installation can be found [HERE](https://github.com/nicolaspernoud/atrium/blob/main/scripts/deploy/up.sh).
Use your own GeoLite2-City.mmdb or remove the line.

In any case you will need to provide a configuration file. Atrium will start without one and create a minimal one, but since it will not even have users, it will be pretty much useless.

Example configuration can be found [HERE](https://github.com/nicolaspernoud/atrium/blob/main/backend/atrium.yaml).
To start quickly : configure your hostname, and set `tls_mode` to `Auto`.

## Configuration

See [atrium.yaml](https://github.com/nicolaspernoud/atrium/blob/main/backend/atrium.yaml) for configuration options and examples.

The `hostname` configuration can be overridden with the environment variable `MAIN_HOSTNAME`.

### DNS

Your DNS configuration should be as below :
|Domain|Type|Target|
|--|--|--|
|your.hostname|A|Your machine IPv4|
|your.hostname|AAAA|Your machine IPv6|
|\*.your.hostname|CNAME|your.hostname|

### Integrated Fail2ban style Jail

Atrium includes an integrated, stateless fail2ban style jail to block IPs that are trying to access files without authorization or failing to authenticate.

It uses `ip(6)tables` to ban IPs on the host machine.

**It would not work with docker deployments, as the docker image does not contain the iptables binaries.**

#### Configuration

The jail is configured in the `jail` section of `atrium.yaml`.

```yaml
jail:
  enabled: true # Enable the fail2ban style jail
  max_retry: 3 # Number of fails before banning the IP
  find_time: 60 # Time window in seconds to count fails
  ban_time: 30 # Ban duration in days
```

#### Prerequisites

- When running in Docker, the container must use the **host network mode** (`network_mode: host`) and have the `NET_ADMIN` capability to alter the host's `iptables`.
- `iptables` must be installed on the host.

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

### Clean useless dependencies and features

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

### Regenerate the frontend

```
mv frontend frontend_old
flutter create --template=app --platforms="android,web" --description="Atrium's frontend app" --org="fr.ninico" --project-name="atrium" frontend
```
