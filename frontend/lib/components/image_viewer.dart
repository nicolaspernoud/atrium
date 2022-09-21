import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:webdav_client/webdav_client.dart';

class ImageViewer extends StatefulWidget {
  const ImageViewer(
      {super.key, required this.client, required this.url, required this.file});

  final Client client;
  final String url;
  final File file;

  @override
  State<ImageViewer> createState() => _ImageViewerState();
}

class _ImageViewerState extends State<ImageViewer> {
  Future<Uint8List>? imgData;

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
                  .then((value) => Uint8List.fromList(value)),
              builder:
                  (BuildContext context, AsyncSnapshot<Uint8List> snapshot) {
                Widget child;
                if (snapshot.hasData) {
                  child = Image.memory(snapshot.data!);
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
