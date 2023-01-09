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
  late WebViewController controller;
  late final WebViewCookieManager cookieManager = WebViewCookieManager();

  @override
  void initState() {
    super.initState();
    controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted);
    initWebView();
  }

  Future<void> initWebView() async {
    await cookieManager.clearCookies();
    await cookieManager.setCookie(WebViewCookie(
        name: authCookieName,
        value: Uri.decodeComponent(App().cookie.split(";")[0].split("=")[1]),
        domain: Uri.parse(widget.initialUrl).host));
    controller.loadRequest(Uri.parse(widget.initialUrl));
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      body: WebViewWidget(
          controller: controller,
          gestureRecognizers: {Factory(() => EagerGestureRecognizer())}),
    );
  }
}
