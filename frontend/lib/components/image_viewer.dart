import 'dart:async';
import 'dart:ui';
import 'package:atrium/components/explorer.dart';
import 'package:atrium/utils.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:photo_view/photo_view.dart';
import 'package:photo_view/photo_view_gallery.dart';
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

  void _onKey(KeyEvent event) {
    _onUserInteraction();
    if (event is KeyDownEvent) {
      if (event.logicalKey == LogicalKeyboardKey.arrowLeft && index > 0) {
        previousPage();
      } else if (event.logicalKey == LogicalKeyboardKey.arrowRight &&
          index < files.length - 1) {
        nextPage();
      }
    }
  }

  @override
  void dispose() {
    pageController.dispose();
    _hideTimer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
        backgroundColor: Colors.black,
        body: KeyboardListener(
          focusNode: FocusNode()..requestFocus(),
          onKeyEvent: _onKey,
          child: MouseRegion(
            onHover: (_) => _onUserInteraction(),
            child: GestureDetector(
              behavior: HitTestBehavior.translucent,
              onTap: _onUserInteraction,
              child: Stack(
                children: [
                  PhotoViewGallery.builder(
                    scrollPhysics: const BouncingScrollPhysics(),
                    builder: (BuildContext context, int index) {
                      return PhotoViewGalleryPageOptions(
                          imageProvider:
                              WebdavImage(files[index], widget.client),
                          initialScale: PhotoViewComputedScale.contained,
                          minScale: PhotoViewComputedScale.contained);
                    },
                    itemCount: files.length,
                    loadingBuilder: (context, event) => Center(
                      child: CircularProgressIndicator(),
                    ),
                    pageController: pageController,
                    onPageChanged: (value) {
                      _onUserInteraction();
                      setState(() {
                        index = value;
                      });
                    },
                  ),

                  // Sliding + Fading AppBar
                  Positioned(
                    top: 0,
                    left: 0,
                    right: 0,
                    child: AnimatedSlide(
                      offset:
                          _controlsVisible ? Offset.zero : const Offset(0, -1),
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
                    bottom: 8.0,
                    left: 0,
                    right: 0,
                    child: AnimatedSlide(
                      offset:
                          _controlsVisible ? Offset.zero : const Offset(0, 1),
                      duration: const Duration(milliseconds: 300),
                      child: AnimatedOpacity(
                        opacity: _controlsVisible ? 1.0 : 0.0,
                        duration: const Duration(milliseconds: 300),
                        child: OverflowBar(
                          alignment: MainAxisAlignment.center,
                          spacing: 8.0,
                          children: [
                            IconButton(
                              style: IconButton.styleFrom(
                                  backgroundColor: Colors.black87),
                              onPressed: index == 0 ? null : previousPage,
                              icon: const Icon(Icons.arrow_left),
                            ),
                            IconButton(
                              style: IconButton.styleFrom(
                                  backgroundColor: Colors.black87),
                              onPressed:
                                  index == files.length - 1 ? null : nextPage,
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
          ),
        ));
  }

  void nextPage() {
    _onUserInteraction();
    pageController.nextPage(
      duration: const Duration(milliseconds: 350),
      curve: Curves.easeInOut,
    );
  }

  void previousPage() {
    _onUserInteraction();
    pageController.previousPage(
      duration: const Duration(milliseconds: 350),
      curve: Curves.easeInOut,
    );
  }
}

@immutable
class WebdavImage extends ImageProvider<File> {
  const WebdavImage(this.file, this.client);

  final File file;
  final Client client;

  @override
  Future<File> obtainKey(ImageConfiguration configuration) {
    return SynchronousFuture<File>(file);
  }

  @override
  ImageStreamCompleter loadImage(File key, ImageDecoderCallback decode) {
    final StreamController<ImageChunkEvent> chunkEvents =
        StreamController<ImageChunkEvent>();
    return MultiFrameImageStreamCompleter(
      codec: _loadAsync(key),
      chunkEvents: chunkEvents.stream,
      scale: 1.0,
      debugLabel: '"key"',
      informationCollector: () => <DiagnosticsNode>[
        DiagnosticsProperty<ImageProvider>('Image provider', this),
        DiagnosticsProperty<File>('URL', key),
      ],
    );
  }

  Future<Codec> _loadAsync(File key) async {
    final Uint8List imageBytes =
        await client.read(key.path!).then((value) => Uint8List.fromList(value));
    final ImmutableBuffer buffer =
        await ImmutableBuffer.fromUint8List(imageBytes);
    return instantiateImageCodecFromBuffer(buffer);
  }

  @override
  String toString() => '${objectRuntimeType(this, 'WebdavImage')}("$file")';
}
