openDocument();

async function openDocument() {
  const config = JSON.parse(
    document.getElementById("OnlyOfficeConfiguration").innerText
  );
  DocsAPI.DocEditor("placeholder", config);
}
