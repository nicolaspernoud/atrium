import 'package:atrium/components/webview.dart'
    if (dart.library.html) 'package:atrium/components/iframe_webview.dart';
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:pdfx/pdfx.dart';
import 'package:webdav_client/webdav_client.dart';

class PdfViewer extends StatefulWidget {
  const PdfViewer(
      {super.key,
      required this.client,
      required this.url,
      required this.file,
      required this.color});

  final Client client;
  final String url;
  final File file;
  final Color color;

  @override
  State<PdfViewer> createState() => _PdfViewerState();
}

class _PdfViewerState extends State<PdfViewer> {
  late PdfController _pdfController;

  @override
  void initState() {
    super.initState();
    if (!isWebDesktop()) {
      _pdfController = PdfController(
          document: PdfDocument.openData(widget.client
              .read(widget.file.path!)
              .then((value) => Uint8List.fromList(value))));
    }
  }

  @override
  void dispose() {
    _pdfController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        backgroundColor: widget.color,
        title: Row(
          children: [
            const Icon(
              Icons.picture_as_pdf,
              size: 30,
            ),
            const SizedBox(width: 15),
            Flexible(
                child: Text(
              widget.file.name!,
              overflow: TextOverflow.ellipsis,
            )),
          ],
        ),
        actions: isWebDesktop()
            ? null
            : <Widget>[
                PdfPageNumber(
                    controller: _pdfController,
                    builder: (_, loadingState, page, pagesCount) =>
                        (pagesCount != null && pagesCount > 0)
                            ? Row(
                                children: [
                                  IconButton(
                                    icon: const Icon(Icons.navigate_before),
                                    onPressed: () {
                                      _pdfController.previousPage(
                                        curve: Curves.ease,
                                        duration:
                                            const Duration(milliseconds: 100),
                                      );
                                    },
                                  ),
                                  Text(
                                    '$page/$pagesCount',
                                    style: const TextStyle(fontSize: 22),
                                  ),
                                  IconButton(
                                    icon: const Icon(Icons.navigate_next),
                                    onPressed: () {
                                      _pdfController.nextPage(
                                        curve: Curves.ease,
                                        duration:
                                            const Duration(milliseconds: 100),
                                      );
                                    },
                                  ),
                                ],
                              )
                            : Container()),
              ],
      ),
      body: isWebDesktop()
          ? FutureBuilder<String>(
              future: ApiProvider()
                  .getShareToken(widget.url.split("://")[1].split(":")[0],
                      widget.file.path!,
                      shareWith: "pdf_viewer", shareForDays: 1)
                  .then((value) =>
                      '${widget.url}${widget.file.path}?token=$value&inline'),
              builder: (BuildContext context, AsyncSnapshot<String> snapshot) {
                Widget child;
                if (snapshot.hasData) {
                  child = AppWebView(
                    initialUrl: snapshot.data!,
                  );
                } else if (snapshot.hasError) {
                  child = Padding(
                    padding: const EdgeInsets.only(top: 16),
                    child: Text('Error: ${snapshot.error}'),
                  );
                } else {
                  child = const SizedBox(
                    width: 60,
                    height: 60,
                    child: CircularProgressIndicator(),
                  );
                }
                return Center(
                  child: child,
                );
              })
          : PdfView(
              builders: PdfViewBuilders<DefaultBuilderOptions>(
                options: const DefaultBuilderOptions(),
                documentLoaderBuilder: (_) =>
                    const Center(child: CircularProgressIndicator()),
                pageLoaderBuilder: (_) =>
                    const Center(child: CircularProgressIndicator()),
                pageBuilder: _pageBuilder,
              ),
              controller: _pdfController,
            ),
    );
  }

  PhotoViewGalleryPageOptions _pageBuilder(
    BuildContext context,
    Future<PdfPageImage> pageImage,
    int index,
    PdfDocument document,
  ) {
    return PhotoViewGalleryPageOptions(
      imageProvider: PdfPageImageProvider(
        pageImage,
        index,
        document.id,
      ),
      minScale: PhotoViewComputedScale.contained * 1,
      maxScale: PhotoViewComputedScale.contained * 2,
      initialScale: PhotoViewComputedScale.contained * 1.0,
      heroAttributes: PhotoViewHeroAttributes(tag: '${document.id}-$index'),
    );
  }
}
