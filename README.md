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

### DNS

Your DNS configuration should be as below :
|Domain|Type|Target|
|--|--|--|
|your.hostname|A|Your machine IPv4|
|your.hostname|AAAA|Your machine IPv6|
|\*.your.hostname|CNAME|your.hostname|

### Fail2ban

To block IPs that are trying to access files without authorization, you can use the provided fail2ban configuration, which runs in a Docker container.

#### Prerequisites

- Docker and Docker Compose must be installed on your system.

#### Installation & Configuration

1.  **Navigate to the fail2ban directory:**

    ```bash
    cd scripts/fail2ban
    ```

2.  **Verify Configuration:**

    - **Container:** Open `docker-compose.yml`. Alter the timezone and ensure the host side of the log volume mount (`/remotelogs/atrium`) points to your actual atrium log directory.
      ```yaml
      environment:
        # ...
        - TZ=Europe/Paris # <- Alter the timezone to match the one of the server
      volumes:
        # ...
        - <path to atrium logs directory>:/remotelogs/atrium # <- Alter this path
      ```
    - **Ignore IPs:** To prevent being locked out, add your own IP addresses to the `ignoreip` list in `jail.local`.
      ```
      ignoreip = 127.0.0.1/8 ::1 YOUR.IP.HERE
      ```

3.  **Start the container:**

    ```bash
    ./up.sh
    ```

The fail2ban service will now monitor the atrium logs and automatically ban IPs that trigger the "FILE ACCESS DENIED" or the "AUTHENTICATION ERROR" rules.
The new logs won't be added automatically, so use the reload.sh script a a crontab to load new log files : `crontab -e` => `10 * * * * /services/fail2ban/reload.sh >/dev/null 2>&1`

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