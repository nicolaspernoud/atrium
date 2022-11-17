openDocument();

async function openDocument() {
  const config = JSON.parse(
    document.getElementById("OnlyOfficeConfiguration").innerText
  );
  new DocsAPI.DocEditor("placeholder", config);
}
