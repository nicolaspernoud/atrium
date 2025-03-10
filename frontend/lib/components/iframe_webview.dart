import 'package:flutter/material.dart';

import 'package:web/web.dart';

import 'package:flutter/widgets.dart';

import 'dart:ui_web' as ui;

class AppWebView extends StatefulWidget {
  final String initialUrl;
  const AppWebView({super.key, required this.initialUrl});

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
        (int id) => HTMLIFrameElement()
          ..style.width = '100%'
          ..style.height = '100%'
          ..src = widget.initialUrl
          ..id = viewId
          ..style.border = 'none');
  }
}
