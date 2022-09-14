import 'dart:io';

import 'package:dio/dio.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:path_provider/path_provider.dart';
import 'package:webdav_client/webdav_client.dart' as webdav;

// Create new client configured for mobile
webdav.Client newExplorerClient(String uri,
    {String user = '', String password = '', bool debug = false}) {
  var client =
      webdav.newClient(uri, user: user, password: password, debug: debug);
  client.auth = webdav.BasicAuth(user: user, pwd: password);
  return client;
}

Dio newDio(BaseOptions options) {
  return Dio(options);
}

download(String url, webdav.Client client, webdav.File file,
    BuildContext context) async {
  String? dir = await getDownloadPath();
  await client
      .read2File(file.path!, '$dir/${file.name!}${file.isDir! ? ".zip" : ""}',
          onProgress: (c, t) {
    // TODO : report progress with notification
    debugPrint((c / t).toString());
  });
}

Future<String?> getDownloadPath() async {
  Directory? directory;
  try {
    if (Platform.isIOS) {
      directory = await getApplicationDocumentsDirectory();
    } else {
      directory = Directory('/storage/emulated/0/Download');
      // Put file in global download folder, if for an unknown reason it didn't exist, we fallback
      if (!await directory.exists()) {
        directory = await getExternalStorageDirectory();
      }
    }
  } catch (err) {
    debugPrint("Cannot get download folder path");
  }
  return directory?.path;
}

upload(String destPath, PlatformFile file, webdav.Client client,
    Function(int, int)? onProgress, CancelToken cancelToken) async {
  await client.writeFromFile(file.path!, "$destPath/${file.name}",
      onProgress: onProgress, cancelToken: cancelToken);
}
