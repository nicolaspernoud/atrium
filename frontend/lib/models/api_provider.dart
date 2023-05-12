import 'package:atrium/models/app.dart';
import 'package:atrium/models/dav.dart';
import 'package:atrium/models/pathitem.dart';
import 'package:atrium/models/sysinfo.dart';
import 'package:atrium/models/user.dart';
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';

import '../globals.dart';

class InterceptorsWrapper extends QueuedInterceptorsWrapper {
  @override
  void onRequest(RequestOptions options, RequestInterceptorHandler handler) {
    if (!kIsWeb) {
      options.headers["cookie"] = App().cookie;
    }
    options.headers["xsrf-token"] = App().xsrfToken;
    super.onRequest(options, handler);
  }

  @override
  void onError(DioError err, ErrorInterceptorHandler handler) {
    if (err.response != null &&
        (err.response!.statusCode == 401 || err.response!.statusCode == 403)) {
      App().cookie = "";
    }
    super.onError(err, handler);
  }
}

class ApiProvider {
  late Dio _dio;

  final BaseOptions options = BaseOptions(
      baseUrl: App().prefs.hostname,
      receiveTimeout: const Duration(seconds: 10),
      contentType: Headers.jsonContentType);
  static final ApiProvider _instance = ApiProvider._internal();

  factory ApiProvider() => _instance;

  ApiProvider._internal() {
    _dio = newDio(options);
    _dio.interceptors.add(InterceptorsWrapper());
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

  Future<bool> hasOIDC() async {
    _dio.options.baseUrl = App().prefs.hostname;
    try {
      await _dio.get('/auth/oauth2login');
    } on DioError catch (e) {
      if (e.response == null ||
          e.response!.statusCode == null ||
          e.response!.statusCode! >= 404) {
        return false;
      }
    }
    return true;
  }

  Future<String?> getShareToken(String hostname, String path,
      {String shareWith = "", int? shareForDays}) async {
    _dio.options.baseUrl = App().prefs.hostname;
    final Map<String, dynamic> request = {
      "hostname": hostname,
      "path": escapePath(path)
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
    await _reloadConfigurationAndWaitUntilReady();
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
    await _reloadConfigurationAndWaitUntilReady();
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
    await _reloadConfigurationAndWaitUntilReady();
  }

  Future<DiskInfo> getDiskInfo(DavModel dav) async {
    final response = await _dio.get('${modelUrl(dav)}?diskusage');
    return DiskInfo.fromJson(response.data);
  }

  Future<List<PathItem>> searchDav(DavModel dav, String query) async {
    final response = await _dio.get('${modelUrl(dav)}?q=$query');
    var pathItemArray = response.data;
    var pathItems = <PathItem>[];
    for (var user in pathItemArray) {
      pathItems.add(PathItem.fromJson(user));
    }
    return pathItems;
  }

  Future<SysInfo> getSysInfo() async {
    final response = await _dio.get('/api/user/system_info');
    return SysInfo.fromJson(response.data);
  }

  Future<Response> _reloadConfigurationAndWaitUntilReady() async {
    await _dio.get('/reload');
    const int maxRetries = 10;
    int retries = 0;
    while (retries < maxRetries) {
      try {
        Response response = await _dio.get("/api/admin/apps");
        return response;
      } catch (e) {
        retries++;
        await Future.delayed(const Duration(seconds: 2));
      }
    }
    throw Exception('Request failed after $maxRetries retries');
  }
}

String modelUrl(Model mdl) {
  if (mdl.host.contains(App().prefs.hostnameHost)) {
    return "${App().prefs.hostnameScheme}://${mdl.host}${App().prefs.hostnamePort != null ? ":${App().prefs.hostnamePort}" : ""}";
  }
  return "${App().prefs.hostnameScheme}://${mdl.host}.${App().prefs.hostnameHost}${App().prefs.hostnamePort != null ? ":${App().prefs.hostnamePort}" : ""}";
}

String escapePath(String path) {
  return Uri.encodeFull(path).replaceAll("'", "%27");
}
