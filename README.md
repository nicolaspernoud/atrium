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
- [ ] OnlyOffice connector for documents editing
- [ ] User authentication and security (OAuth2)
- [ ] OnlyOffice : custom logo / favicon / title
- [ ] Allow working behind https proxy

### Frontend

- [ ] Dio interceptor (Future already completed and 403)
- [ ] Improve upload (refresh on each upload ending, better view)
- [ ] Confirm dialog for deletes

- [ ] Use research file capability
- [ ] Error handling (fix uncaught exceptions)

- [ ] Sound files displaying
- [ ] Video files displaying

- [ ] OnlyOffice connector for documents editing
