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
  void initState() {
    super.initState();
    getFileContent();
  }

  Future<void> getFileContent() async {
    var content = await widget.client.read(widget.file.path!);
    setState(() {
      pdfData = Future.value(Uint8List.fromList(content));
      pdfController = PdfControllerPinch(
        document: PdfDocument.openData(pdfData!),
      );
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.file.name!),
      ),
      body: Center(
          child: FutureBuilder<Uint8List>(
              future: pdfData,
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
