import 'dart:io';
import 'package:atrium/globals.dart';
import 'package:atrium/platform/mobile.dart';
import 'package:atrium/utils.dart';
import 'package:file_picker/file_picker.dart' as file_picker;
import 'package:flutter/foundation.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/material.dart';
import 'package:webview_flutter/webview_flutter.dart';
import 'package:webview_flutter_android/webview_flutter_android.dart';

const authCookieName = "ATRIUM_AUTH";

class AppWebView extends StatefulWidget {
  late final String initialUrl;
  // ignore: prefer_const_constructors_in_immutables
  AppWebView({super.key, required this.initialUrl});

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
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..setNavigationDelegate(NavigationDelegate(
        onNavigationRequest: (NavigationRequest request) {
          var urlWithoutQuery = request.url.split('?').first;
          var fileName = Uri.decodeFull(urlWithoutQuery.split('/').last);
          if (fileTypeFromExt(fileName.split(".").last) != FileType.other) {
            webDownload(request.url, fileName);
            return NavigationDecision.prevent;
          }
          return NavigationDecision.navigate;
        },
      ));
    if (Platform.isAndroid) {
      final androidController = controller.platform as AndroidWebViewController;
      androidController.setOnShowFileSelector((params) async {
        file_picker.FilePickerResult? result =
            await file_picker.FilePicker.platform.pickFiles();
        if (result != null && result.files.single.path != null) {
          File file = File(result.files.single.path!);
          return [file.uri.toString()];
        }
        return [];
      });
    }
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
