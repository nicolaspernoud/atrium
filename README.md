# atrium

Atrium is a web server, reverse proxy and webdav server with user authentication. It comes with a multiplatform client application.

Rust/Flutter version of Vestibule.

!!! WORK IN PROGRESS !!!

## TODO

### Backend

- [ ] Remove clones, panics, expects, unwraps, println!, etc.
- [ ] Lifetimes for non serialized structs
- [ ] Litmus compliance in CI tests
- [ ] Remove axum macros
- [ ] Check that there is enough tests
- [ ] OnlyOffice connector for documents editing
- [ ] Reduce compiled file size (strip symbols, etc.)
- [ ] User authentication and security (OAuth2)
- [ ] Remove anyhow if possible

### Frontend

- [ ] Download progress on mobile
- [ ] Improve upload (refresh on each upload ending, better view)
- [ ] Confirm dialog for deletes

- [ ] Use research file capability
- [ ] Error handling (fix uncaught exceptions)

- [ ] Sound files displaying
- [ ] Video files displaying
- [ ] Image previews in explorer

- [ ] OnlyOffice connector for documents editing
