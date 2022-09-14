import 'dart:io';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:atrium/models/preferences.dart';
import 'package:flutter/material.dart';
import 'package:webdav_client/webdav_client.dart';

import 'components/uploads.dart';

class App extends ChangeNotifier {
  late Preferences prefs;
  App._privateConstructor();
  final Uploads _uploads = Uploads();

  static final App _instance = App._privateConstructor();

  factory App() {
    return _instance;
  }

  bool get hasToken {
    return prefs.cookie != "";
  }

  String get cookie {
    return prefs.cookie;
  }

  String get token {
    return prefs.cookie.split("=")[1];
  }

  set cookie(String cookie) {
    prefs.cookie = cookie;
    notifyListeners();
  }

  String get xsrfToken {
    return prefs.xsrfToken;
  }

  set xsrfToken(String xsrfToken) {
    prefs.xsrfToken = xsrfToken;
  }

  bool get isAdmin {
    return prefs.isAdmin;
  }

  set isAdmin(bool isAdmin) {
    prefs.isAdmin = isAdmin;
    notifyListeners();
  }

  List<Upload> get uploads {
    return _uploads.uploads;
  }

  bool get hasUploads {
    return _uploads.uploads.isNotEmpty;
  }

  void pushUpload(Client client, PlatformFile file, String destPath) {
    _uploads.push(client, file, destPath);
    notifyListeners();
  }

  void removeUpload(Upload upload) {
    _uploads.uploads.removeWhere((element) => element == upload);
  }

  Future<Upload?> uploadOne() async {
    return await _uploads.uploadOne();
  }

  void reportProgress() {
    notifyListeners();
  }

  log(String v) async {
    await prefs.addToLog(v);
  }

  getLog() {
    return prefs.log;
  }

  clearLog() {
    prefs.clearLog();
  }

  Future init() async {
    prefs = Preferences();
    if (kIsWeb || !Platform.environment.containsKey('FLUTTER_TEST')) {
      await prefs.read();
    }
  }
}
