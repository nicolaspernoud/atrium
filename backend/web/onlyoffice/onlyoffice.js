openDocument();

async function openDocument() {
  const urlParams = new URLSearchParams(window.location.search);
  const file = urlParams.get("file");
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
  name = name.replace(/[\[]/, '\\[').replace(/[\]]/, '\\]');
  var regex = new RegExp('[\\?&]' + name + '=([^&#]*)');
  var results = regex.exec(location.search);
  return results === null ? '' : results[1];
};