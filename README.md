# atrium

Atrium is a web server, reverse proxy and webdav server with user authentication. It comes with a multiplatform client application.

Rust/Flutter version of Vestibule.

!!! WORK IN PROGRESS !!!

## TODO

### Backend

- [ ] Remove clones, panics, expects, unwraps, println!, etc.
- [ ] Lifetimes for non serialized structs
- [ ] Remove axum macros
- [ ] Check that there is enough tests
- [ ] Add OnlyOffice JWT security

### Frontend

- [ ] Improve upload (refresh on each upload ending, better view)
- [ ] Use research file capability
- [ ] Error handling (fix uncaught exceptions)
