window.opener.postMessage(window.location.href, window.location.origin);
window.opener.postMessage(
  window.location.href,
  "http://atrium.127.0.0.1.nip.io:3000/"
); // Debug mode
window.open("", "_self").close();
