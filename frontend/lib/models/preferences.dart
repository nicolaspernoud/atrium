import 'dart:convert';

import 'dart:async';
import 'dart:io';

import 'package:flutter/foundation.dart' show kDebugMode, kIsWeb;
import 'package:path_provider/path_provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

var urlRegExp = RegExp(r"(https|http)?(:\/\/)?([^:]*):?(\d*)?");

class Preferences with LocalFilePersister {
  Preferences();

  String _hostname = kDebugMode ? "http://atrium.127.0.0.1.nip.io:8080" : "";

  set hostname(String v) {
    _hostname = v;
    write();
  }

  String get hostname => _hostname;
  String get hostnameScheme =>
      urlRegExp.allMatches(_hostname).first.group(1) ?? "https";
  String get hostnameHost =>
      urlRegExp.allMatches(_hostname).first.group(3) ?? "atrium.io";
  int? get hostnamePort {
    var portString = urlRegExp.allMatches(_hostname).first.group(4);
    return portString != null ? int.parse(portString) : null;
  }

  String _username = "";

  set username(String v) {
    _username = v;
    write();
  }

  String get username => _username;

  String _cookie = "";

  set cookie(String v) {
    _cookie = v;
    write();
  }

  String get cookie => _cookie;

  String _xsrfToken = "";

  set xsrfToken(String v) {
    _xsrfToken = v;
    write();
  }

  String get xsrfToken => _xsrfToken;

  bool _isAdmin = false;

  set isAdmin(bool v) {
    _isAdmin = v;
    write();
  }

  bool get isAdmin => _isAdmin;

  bool _logEnabled = false;

  set logEnabled(bool v) {
    _logEnabled = v;
    write();
  }

  bool get logEnabled => _logEnabled;

  List<String> _log = [""];

  addToLog(String v) async {
    if (_logEnabled) {
      _log.add("${formatTime(DateTime.now())} - $v");
      await write();
    }
  }

  List<String> get log => _log;

  clearLog() {
    _log.clear();
    write();
  }

  @override
  fromJson(String source) {
    Map settingsMap = jsonDecode(source);
    _hostname = settingsMap['hostname'];
    _username = settingsMap['username'];
    _cookie = settingsMap['cookie'];
    _xsrfToken = settingsMap['xsrfToken'];
    _isAdmin = settingsMap['isAdmin'];
    _logEnabled = settingsMap['logEnabled'];
    _log = List<String>.from(settingsMap['log']);
  }

  @override
  String toJson() {
    Map<String, dynamic> settingsMap = {
      'hostname': _hostname,
      'username': _username,
      'cookie': _cookie,
      'xsrfToken': _xsrfToken,
      'isAdmin': _isAdmin,
      'logEnabled': _logEnabled,
      'log': _log
    };
    return jsonEncode(settingsMap);
  }
}

mixin LocalFilePersister {
  fromJson(String source);
  toJson();

  // Persistence
  final String _fileName = "settings.json";

  Future<File> get localFile async {
    if (Platform.isAndroid) {
      final directory = await getExternalStorageDirectory();
      await Directory('${directory?.path}').create(recursive: true);
      return File('${directory?.path}/$_fileName');
    }
    final directory = await getApplicationDocumentsDirectory();
    return File('${directory.path}/$_fileName');
  }

  read() async {
    if (kIsWeb) {
      SharedPreferences prefs = await SharedPreferences.getInstance();
      String? contents = prefs.getString("settings");
      if (contents != null) fromJson(contents);
    } else {
      try {
        final file = await localFile;
        String contents = await file.readAsString();
        fromJson(contents);
      } catch (e) {
        // ignore: avoid_print
        print("data could not be loaded from file, defaulting to new data");
      }
    }
  }

  write() async {
    if (kIsWeb) {
      SharedPreferences prefs = await SharedPreferences.getInstance();
      prefs.setString("settings", toJson());
    } else {
      final file = await localFile;
      file.writeAsString(toJson());
    }
  }
}

String formatTime(DateTime d) {
  return "${d.year.toString()}-${d.month.toString().padLeft(2, "0")}-${d.day.toString().padLeft(2, "0")} ${d.hour.toString().padLeft(2, "0")}:${d.minute.toString().padLeft(2, "0")}:${d.second.toString().padLeft(2, "0")}";
}
