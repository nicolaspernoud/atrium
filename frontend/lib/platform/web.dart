// ignore: avoid_web_libraries_in_flutter
import 'dart:html';

import 'package:atrium/globals.dart';
import 'package:dio/adapter_browser.dart';
import 'package:dio/dio.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:webdav_client/webdav_client.dart' as webdav;
import 'package:atrium/models/api_provider.dart';

// Create new client configured for web
webdav.Client newExplorerClient(String uri,
    {String user = '', String password = '', bool debug = false}) {
  var client =
      webdav.newClient(uri, user: user, password: password, debug: debug);
  var adapter = BrowserHttpClientAdapter();
  adapter.withCredentials = true;
  client.c.httpClientAdapter = adapter;
  client.c.interceptors.add(QueuedInterceptorsWrapper(
    onRequest: (
      RequestOptions requestOptions,
      RequestInterceptorHandler handler,
    ) {
      requestOptions.headers["xsrf-token"] = App().xsrfToken;
      handler.next(requestOptions);
    },
  ));
  return client;
}

Dio newDio(BaseOptions options) {
  var dio = Dio(options);
  var adapter = BrowserHttpClientAdapter();
  adapter.withCredentials = true;
  dio.httpClientAdapter = adapter;
  return dio;
}

download(String url, webdav.Client client, webdav.File file,
    BuildContext context) async {
  var shareToken = await ApiProvider()
      .getShareToken(url.split("://")[1].split(":")[0], file.path!);
  AnchorElement()
    ..href = '$url${escapePath(file.path!)}?token=$shareToken'
    ..click();
}

upload(String destPath, PlatformFile file, webdav.Client client,
    Function(int, int)? onProgress, CancelToken cancelToken) async {
  var path = "$destPath${file.name}";
  await client.c.wdWriteWithStream(
    client,
    path,
    file.readStream!,
    file.size,
    onProgress: onProgress,
    cancelToken: cancelToken,
  );
}
