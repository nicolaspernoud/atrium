# atrium

Atrium is a web server, reverse proxy and webdav server with user authentication. It comes with a multiplatform client application.

Rust/Flutter version of Vestibule.

## Configuration

See atrium.yaml for configuration options and examples.

The `hostname` configuration can be overridden with the environment variable `MAIN_HOSTNAME`.

## TODO

### Backend

- [ ] Move OnlyOffice JWT to backend
- [ ] Performance tests and improvements

### Frontend

- [ ] Improve upload (refresh on each upload ending, better view)
- [ ] Use research file capability
- [ ] Error handling (fix uncaught exceptions)
