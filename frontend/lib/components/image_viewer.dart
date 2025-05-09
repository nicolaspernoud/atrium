import 'dart:async';
import 'dart:math';
import 'dart:typed_data';
import 'package:atrium/components/explorer.dart';
import 'package:atrium/utils.dart';
import 'package:flutter/material.dart';
import 'package:webdav_client/webdav_client.dart';

class ImageViewer extends StatefulWidget {
  const ImageViewer({
    super.key,
    required this.client,
    required this.url,
    required this.files,
    required this.index,
  });

  final Client client;
  final String url;
  final List<File> files;
  final int index;

  @override
  State<ImageViewer> createState() => _ImageViewerState();
}

class _ImageViewerState extends State<ImageViewer> {
  late int index;
  late List<File> files;
  late PageController pageController;
  PageView? pageView;

  Future<Uint8List>? previousImage;
  Future<Uint8List>? currentImage;
  Future<Uint8List>? nextImage;

  bool _controlsVisible = true;
  Timer? _hideTimer;

  @override
  void initState() {
    super.initState();
    index = widget.index;
    var baseIndex = index;
    for (var i = 0; i <= baseIndex; i++) {
      if (fileType(widget.files[i]) != FileType.image) index--;
    }
    files = widget.files.where((f) => fileType(f) == FileType.image).toList();
    pageController = PageController(initialPage: index);
    _startHideTimer();
  }

  void _startHideTimer() {
    _hideTimer?.cancel();
    _hideTimer = Timer(const Duration(seconds: 2), () {
      setState(() {
        _controlsVisible = false;
      });
    });
  }

  void _onUserInteraction() {
    if (!_controlsVisible) {
      setState(() => _controlsVisible = true);
    }
    _startHideTimer();
  }

  Future<Uint8List>? getImage(int i) {
    if (i == index) {
      currentImage = widget.client
          .read(files[index].path!)
          .then((value) => Uint8List.fromList(value));
      previousImage = index > 0
          ? widget.client
              .read(files[i - 1].path!)
              .then((value) => Uint8List.fromList(value))
          : null;
      nextImage = index <= files.length - 2
          ? widget.client
              .read(files[i + 1].path!)
              .then((value) => Uint8List.fromList(value))
          : null;
    } else if (i < index) {
      index = max(i, 0);
      // Backward
      nextImage = currentImage;
      currentImage = previousImage;
      previousImage = i > 0
          ? widget.client
              .read(files[i - 1].path!)
              .then((value) => Uint8List.fromList(value))
          : null;
    } else {
      // Forward
      index = min(i, files.length - 1);
      previousImage = currentImage;
      currentImage = nextImage;
      nextImage = i <= files.length - 2
          ? widget.client
              .read(files[i + 1].path!)
              .then((value) => Uint8List.fromList(value))
          : null;
    }
    return currentImage;
  }

  @override
  void dispose() {
    pageController.dispose();
    _hideTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Build it only once
    pageView ??= PageView.builder(
      controller: pageController,
      itemCount: files.length,
      itemBuilder: (BuildContext context, int i) {
        return FutureBuilder<Uint8List>(
          future: getImage(i),
          builder: (BuildContext context, AsyncSnapshot<Uint8List> snapshot) {
            Widget child;
            if (snapshot.hasData &&
                snapshot.connectionState == ConnectionState.done) {
              child = InteractiveViewer(
                child: Image.memory(snapshot.data!),
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
                child: Center(child: CircularProgressIndicator()),
              );
            }
            return child;
          },
        );
      },
      onPageChanged: (value) {
        setState(() {});
      },
    );

    return Scaffold(
      backgroundColor: Colors.black,
      body: GestureDetector(
        behavior: HitTestBehavior.translucent,
        onTap: _onUserInteraction,
        onPanDown: (_) => _onUserInteraction(),
        child: Stack(
          children: [
            pageView!,
            // Sliding + Fading AppBar
            Positioned(
              top: 0,
              left: 0,
              right: 0,
              child: AnimatedSlide(
                offset: _controlsVisible ? Offset.zero : const Offset(0, -1),
                duration: const Duration(milliseconds: 300),
                child: AnimatedOpacity(
                  opacity: _controlsVisible ? 1.0 : 0.0,
                  duration: const Duration(milliseconds: 300),
                  child: AppBar(
                    backgroundColor: Colors.black87,
                    title: Text(files[index].name ?? ''),
                  ),
                ),
              ),
            ),

            // Sliding + Fading Bottom Bar
            Positioned(
              bottom: 0,
              left: 0,
              right: 0,
              child: AnimatedSlide(
                offset: _controlsVisible ? Offset.zero : const Offset(0, 1),
                duration: const Duration(milliseconds: 300),
                child: AnimatedOpacity(
                  opacity: _controlsVisible ? 1.0 : 0.0,
                  duration: const Duration(milliseconds: 300),
                  child: OverflowBar(
                    alignment: MainAxisAlignment.center,
                    children: [
                      IconButton(
                        onPressed: index == 0
                            ? null
                            : () {
                                pageController.previousPage(
                                  duration: const Duration(milliseconds: 350),
                                  curve: Curves.easeInOut,
                                );
                              },
                        icon: const Icon(Icons.arrow_left),
                      ),
                      IconButton(
                        onPressed: index == files.length - 1
                            ? null
                            : () {
                                pageController.nextPage(
                                  duration: const Duration(milliseconds: 350),
                                  curve: Curves.easeInOut,
                                );
                              },
                        icon: const Icon(Icons.arrow_right),
                      ),
                    ],
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
