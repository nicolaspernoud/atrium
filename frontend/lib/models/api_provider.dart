import 'package:atrium/models/app.dart';
import 'package:atrium/models/dav.dart';
import 'package:atrium/models/sysinfo.dart';
import 'package:atrium/models/user.dart';
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';

import '../globals.dart';

class ApiProvider {
  late Dio _dio;

  final BaseOptions options = BaseOptions(
    baseUrl: App().prefs.hostname,
    connectTimeout: 15000,
    receiveTimeout: 13000,
  );
  static final ApiProvider _instance = ApiProvider._internal();

  factory ApiProvider() => _instance;

  ApiProvider._internal() {
    _dio = newDio(options);
    _dio.interceptors.add(QueuedInterceptorsWrapper(onRequest: (
      RequestOptions requestOptions,
      RequestInterceptorHandler handler,
    ) {
      if (!kIsWeb) {
        requestOptions.headers["cookie"] = App().cookie;
      }
      requestOptions.headers["xsrf-token"] = App().xsrfToken;
      handler.next(requestOptions);
    }, onError: (DioError e, handler) async {
      if (e.response != null) {
        if (e.response!.statusCode == 401) {
          App().cookie = "";
        }
        handler.next(e);
      }
    }));
  }

  Future login(String login, String password) async {
    _dio.options.baseUrl = App().prefs.hostname;
    final request = {"login": login, "password": password};
    final response = await _dio.post('/auth/local', data: request);
    final cookies = response.headers.map['set-cookie'];
    if (response.statusCode == 200) {
      if (cookies != null && cookies.isNotEmpty) {
        final authCookie = cookies[0];
        App().cookie = authCookie.split(";")[0];
      } else {
        App().cookie = "ATRIUM_AUTH=DUMMY_COOKIE_REAL_ONE_FROM_BROWSER";
      }
      App().isAdmin = response.data["is_admin"];
      App().xsrfToken = response.data["xsrf_token"];
      App().prefs.username = login;
    }
  }

  Future<String?> getShareToken(String hostname, String path,
      {String shareWith = "", int? shareForDays}) async {
    _dio.options.baseUrl = App().prefs.hostname;
    final Map<String, dynamic> request = {
      "hostname": hostname,
      "path": Uri.encodeFull(path)
    };
    if (shareForDays != null) {
      request.addAll({"share_with": shareWith, "share_for_days": shareForDays});
    }
    final response =
        await _dio.post('/api/user/get_share_token', data: request);
    return response.data.split("=")[1];
  }

  Future<List<AppModel>> listApps() async {
    final response = await _dio.get('/api/user/list_services');
    var appArray = response.data[0];
    var apps = <AppModel>[];
    for (var app in appArray) {
      apps.add(AppModel.fromJson(app));
    }
    return apps;
  }

  Future<List<DavModel>> listDavs() async {
    final response = await _dio.get('/api/user/list_services');
    var davArray = response.data[1];
    var davs = <DavModel>[];
    for (var dav in davArray) {
      davs.add(DavModel.fromJson(dav));
    }
    return davs;
  }

  Future<List<AppModel>> getApps() async {
    final response = await _dio.get('/api/admin/apps');
    var appArray = response.data;
    var apps = <AppModel>[];
    for (var app in appArray) {
      apps.add(AppModel.fromJson(app));
    }
    return apps;
  }

  Future<void> deleteApp(int id) async {
    await _dio.delete('/api/admin/apps/$id');
    await _dio.get('/reload');
  }

  Future<void> createApp(AppModel app) async {
    await _dio.post('/api/admin/apps', data: app);
    await _dio.get('/reload');
  }

  Future<List<DavModel>> getDavs() async {
    final response = await _dio.get('/api/admin/davs');
    var davArray = response.data;
    var davs = <DavModel>[];
    for (var dav in davArray) {
      davs.add(DavModel.fromJson(dav));
    }
    return davs;
  }

  Future<void> deleteDav(int id) async {
    await _dio.delete('/api/admin/davs/$id');
    await _dio.get('/reload');
  }

  Future<void> createDav(DavModel dav) async {
    await _dio.post('/api/admin/davs', data: dav);
    await _dio.get('/reload');
  }

  Future<List<UserModel>> getUsers() async {
    final response = await _dio.get('/api/admin/users');
    var userArray = response.data;
    var users = <UserModel>[];
    for (var user in userArray) {
      users.add(UserModel.fromJson(user));
    }
    return users;
  }

  Future<void> deleteUser(String login) async {
    await _dio.delete('/api/admin/users/$login');
    await _dio.get('/reload');
  }

  Future<void> createUser(UserModel user) async {
    await _dio.post('/api/admin/users', data: user);
    await _dio.get('/reload');
  }

  Future<DiskInfo> getDiskInfo(DavModel dav) async {
    final response = await _dio.get('${modelUrl(dav)}?diskusage');
    return DiskInfo.fromJson(response.data);
  }

  Future<SysInfo> getSysInfo() async {
    final response = await _dio.get('/api/user/system_info');
    return SysInfo.fromJson(response.data);
  }
}

String modelUrl(Model mdl) {
  return "${App().prefs.hostnameScheme}://${mdl.host}.${App().prefs.hostnameHost}${App().prefs.hostnamePort != null ? ":${App().prefs.hostnamePort}" : ""}";
}
