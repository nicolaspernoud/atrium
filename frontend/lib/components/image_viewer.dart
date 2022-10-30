import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:mime/mime.dart';
import 'package:webdav_client/webdav_client.dart';

class ImageViewer extends StatefulWidget {
  const ImageViewer(
      {super.key,
      required this.client,
      required this.url,
      required this.files,
      required this.index});

  final Client client;
  final String url;
  final List<File> files;
  final int index;

  @override
  State<ImageViewer> createState() => _ImageViewerState();
}

class _ImageViewerState extends State<ImageViewer> {
  late int index;
  late File file;

  @override
  void initState() {
    index = widget.index;
    file = widget.files[index];
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(file.name!), actions: [
        IconButton(
            onPressed: () {
              seekImage(false);
            },
            icon: const Icon(Icons.arrow_left)),
        IconButton(
            onPressed: () {
              seekImage(true);
            },
            icon: const Icon(Icons.arrow_right))
      ]),
      body: GestureDetector(
        onHorizontalDragEnd: (DragEndDetails details) {
          if (details.primaryVelocity! > 0) {
            seekImage(false);
          } else if (details.primaryVelocity! < 0) {
            seekImage(true);
          }
        },
        child: Center(
            child: FutureBuilder<Uint8List>(
                future: widget.client
                    .read(file.path!)
                    .then((value) => Uint8List.fromList(value)),
                builder:
                    (BuildContext context, AsyncSnapshot<Uint8List> snapshot) {
                  Widget child;
                  if (snapshot.hasData &&
                      snapshot.connectionState == ConnectionState.done) {
                    child = Image.memory(
                      snapshot.data!,
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
                  return AnimatedSwitcher(
                    duration: const Duration(milliseconds: 250),
                    child: child,
                  );
                })),
      ),
    );
  }

  void seekImage(bool forward) {
    var i = forward ? index + 1 : index - 1;
    while (i >= 0 && i < widget.files.length) {
      if (lookupMimeType(widget.files[i].name!)?.contains("image") ?? false) {
        setState(() {
          index = i;
          file = widget.files[index];
        });
        break;
      }
      i = forward ? i + 1 : i - 1;
    }
  }
}
