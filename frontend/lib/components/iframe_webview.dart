import 'package:flutter/material.dart';

// ignore: avoid_web_libraries_in_flutter
import 'dart:html';

import 'package:flutter/widgets.dart';

import 'dart:ui' as ui;

class AppWebView extends StatefulWidget {
  final String initialUrl;
  const AppWebView({Key? key, required this.initialUrl}) : super(key: key);

  @override
  State<AppWebView> createState() => _AppWebViewState();
}

class _AppWebViewState extends State<AppWebView> {
  @override
  void initState() {
    super.initState();
    _initIframeView();
  }

  @override
  Widget build(BuildContext context) {
    return HtmlElementView(
      viewType: viewId.toString(),
    );
  }

  String viewId = UniqueKey().toString();

  void _initIframeView() {
    // ignore: undefined_prefixed_name
    ui.platformViewRegistry.registerViewFactory(
        viewId.toString(),
        (int id) => IFrameElement()
          ..width = MediaQuery.of(context).size.width.toString()
          ..height = MediaQuery.of(context).size.height.toString()
          ..src = widget.initialUrl
          ..id = viewId
          ..style.border = 'none');
  }
}
