import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:pdfx/pdfx.dart';
import 'package:webdav_client/webdav_client.dart';

class PdfViewer extends StatefulWidget {
  const PdfViewer(
      {super.key, required this.client, required this.url, required this.file});

  final Client client;
  final String url;
  final File file;

  @override
  State<PdfViewer> createState() => _PdfViewerState();
}

class _PdfViewerState extends State<PdfViewer> {
  Future<Uint8List>? pdfData;
  late PdfControllerPinch pdfController;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.file.name!),
      ),
      body: Center(
          child: FutureBuilder<Uint8List>(
              future: widget.client
                  .read(widget.file.path!)
                  .then((value) => Uint8List.fromList(value))
                  .then((value) {
                pdfController = PdfControllerPinch(
                  document: PdfDocument.openData(value),
                );
                return value;
              }),
              builder:
                  (BuildContext context, AsyncSnapshot<Uint8List> snapshot) {
                Widget child;
                if (snapshot.hasData) {
                  child = PdfViewPinch(
                    controller: pdfController,
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
              })),
    );
  }
}
