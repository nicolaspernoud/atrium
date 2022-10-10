openDocument();

async function openDocument() {
  const urlParams = new URLSearchParams(window.location.search);
  const file = getRawUrlParameter("file");
  const token = getRawUrlParameter("token");
  const url = `${file}?token=${token}`;
  const user = urlParams.get("user");
  const mtime = urlParams.get("mtime");
  const fileName = urlParams.get("file").split("/").pop();
  const fileExtension = file.split(".").pop();
  const key = (await digestMessage(fileName + mtime)).substring(0, 20);
  const config = {
    document: {
      fileType: fileExtension,
      key: key,
      title: fileName,
      url: url,
    },
    editorConfig: {
      lang: "fr-FR",
      callbackUrl: `${document.getElementById("hostname").innerText
        }/onlyoffice/save?${url}`,
      customization: {
        autosave: false,
      },
      user: {
        id: user,
        name: user,
      },
    },
  };
  // Transform config into JWT
  config.token = await createJWT(config);
  // Pass the jwt to document editor
  new DocsAPI.DocEditor("placeholder", config);
}

async function digestMessage(message) {
  const msgUint8 = new TextEncoder().encode(message);
  const hashBuffer = await crypto.subtle.digest("SHA-256", msgUint8);
  const hashArray = Array.from(new Uint8Array(hashBuffer));
  return hashArray.map((b) => b.toString(16).padStart(2, "0")).join("");
}

function EncodeURIWithSpecialsCharacters(str) {
  return encodeURI(str).replace(/[!'()*]/g, encodeURIComponent);
}

function getRawUrlParameter(name) {
  name = name.replace(/[\[]/, "\\[").replace(/[\]]/, "\\]");
  var regex = new RegExp("[\\?&]" + name + "=([^&#]*)");
  var results = regex.exec(location.search);
  return results === null ? "" : results[1];
}

async function createJWT(payload) {
  const header = {
    alg: "HS256",
    typ: "JWT",
  };
  const secret = document.getElementById("jwt_secret").innerText;
  const algorithm = { name: "HMAC", hash: "SHA-256" };
  const payloadAsJSON = JSON.stringify(payload);
  var headerAsJSON = JSON.stringify(header);
  var partialToken = Base64URL.stringify(utf8ToUint8Array(headerAsJSON)) + '.' +
    Base64URL.stringify(utf8ToUint8Array(payloadAsJSON));
  var keyData = utf8ToUint8Array(secret);

  const key = await crypto.subtle.importKey(
    'raw',
    keyData,
    algorithm,
    false,
    ['sign']
  );

  var messageAsUint8Array = utf8ToUint8Array(partialToken);

  const signature = await crypto.subtle.sign(
    'HMAC',
    key,
    messageAsUint8Array
  )

  var signatureAsBase64 = Base64URL.stringify(new Uint8Array(signature));
  return partialToken + '.' + signatureAsBase64;
}

// Adapted from https://chromium.googlesource.com/chromium/blink/+/master/LayoutTests/crypto/subtle/hmac/sign-verify.html
var Base64URL = {
  stringify: function (a) {
    var base64string = btoa(String.fromCharCode.apply(0, a));
    return base64string.replace(/=/g, '').replace(/\+/g, '-').replace(/\//g, '_');
  },
  parse: function (s) {
    s = s.replace(/-/g, '+').replace(/_/g, '/').replace(/\s/g, '');
    return new Uint8Array(Array.prototype.map.call(atob(s), function (c) { return c.charCodeAt(0); }));
  }
};

function utf8ToUint8Array(str) {
  str = window.btoa(decodeURIComponent(encodeURIComponent(str)));
  return Base64URL.parse(str);
}