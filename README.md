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

## TODO

### Frontend

- [ ] Improve upload (refresh on each upload ending, better view)
- [ ] Use research file capability
- [ ] Error handling (fix uncaught exceptions)
