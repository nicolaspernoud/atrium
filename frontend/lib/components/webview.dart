import 'dart:io';

import 'package:atrium/globals.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';

const authCookieName = "ATRIUM_AUTH";

class AppWebView extends StatefulWidget {
  late final String initialUrl;
  // ignore: prefer_const_constructors_in_immutables
  AppWebView({Key? key, required this.initialUrl}) : super(key: key);

  @override
  AppWebViewState createState() => AppWebViewState();
}

class AppWebViewState extends State<AppWebView> {
  final cookieManager = CookieManager();

  @override
  void initState() {
    super.initState();
    // Enable virtual display.
    if (Platform.isAndroid) WebView.platform = AndroidWebView();
  }

  @override
  Widget build(BuildContext context) {
    cookieManager.clearCookies();
    var authCookie = WebViewCookie(
        name: authCookieName,
        value: Uri.decodeComponent(App().cookie.split(";")[0].split("=")[1]),
        domain: Uri.parse(widget.initialUrl).host);

    return WebView(
        initialCookies: [authCookie],
        initialUrl: widget.initialUrl,
        javascriptMode: JavascriptMode.unrestricted,
        gestureRecognizers: Platform.isAndroid
            ? {Factory(() => EagerGestureRecognizer())}
            : null);
  }
}
