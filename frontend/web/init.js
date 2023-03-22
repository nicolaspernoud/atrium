// The value below is altered by the docker image build
let serviceWorkerVersion = null;

pdfjsLib.GlobalWorkerOptions.workerSrc =
  "https://cdn.jsdelivr.net/npm/pdfjs-dist@2.12.313/build/pdf.worker.min.js";
window.pdfRenderOptions = {
  cMapUrl: "https://cdn.jsdelivr.net/npm/pdfjs-dist@2.12.313/cmaps/",
  cMapPacked: true,
};

window.addEventListener("load", function () {
  let loading = document.querySelector("#loading");
  _flutter.loader
    .loadEntrypoint({
      serviceWorker: {
        serviceWorkerVersion: serviceWorkerVersion,
      },
    })
    .then(function (engineInitializer) {
      return engineInitializer.initializeEngine();
    })
    .then(function (appRunner) {
      loading.classList.add("init_done");
      return appRunner.runApp();
    })
    .then(function (app) {
      // Wait a few milliseconds so users can see the "zoom" animation
      // before getting rid of the "loading" div.
      window.setTimeout(function () {
        loading.remove();
      }, 200);
    });
});
