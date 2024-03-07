import 'dart:io';
import 'dart:math';
import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:dio/dio.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_local_notifications/flutter_local_notifications.dart';
import 'package:path_provider/path_provider.dart';
import 'package:webdav_client/webdav_client.dart' as webdav;
import 'package:webview_cookie_manager/webview_cookie_manager.dart';
import 'package:webview_flutter/webview_flutter.dart';

class NotificationsPlugin {
  late FlutterLocalNotificationsPlugin flip;

  static final NotificationsPlugin _instance = NotificationsPlugin._internal();

  factory NotificationsPlugin() => _instance;

  NotificationsPlugin._internal() {
    flip = FlutterLocalNotificationsPlugin();
    var android =
        const AndroidInitializationSettings('@drawable/notification_icon');
    var settings = InitializationSettings(android: android);
    flip.initialize(settings);
  }

  Future showSimpleNotification(String title, String message) async {
    var androidPlatformChannelSpecifics = const AndroidNotificationDetails(
        'atrium-id', 'atrium',
        channelDescription: 'atrium-channel',
        importance: Importance.max,
        priority: Priority.high,
        color: Colors.indigo,
        enableVibration: false);
    var platformChannelSpecifics =
        NotificationDetails(android: androidPlatformChannelSpecifics);
    await flip.show(0, title, message, platformChannelSpecifics,
        payload: 'Default_Sound');
  }

  Future showProgressNotification(String title, String body, int progressId,
      int currentProgress, int maxProgress) async {
    final AndroidNotificationDetails androidNotificationDetails =
        AndroidNotificationDetails('progress channel', 'progress channel',
            channelDescription: 'progress channel description',
            channelShowBadge: false,
            importance: Importance.max,
            priority: Priority.high,
            onlyAlertOnce: true,
            showProgress: true,
            maxProgress: maxProgress,
            progress: currentProgress,
            playSound: false,
            enableVibration: false);
    final NotificationDetails notificationDetails =
        NotificationDetails(android: androidNotificationDetails);
    await flip.show(progressId, title, body, notificationDetails,
        payload: 'item x');
  }
}

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
  var downloadingTitle = tr(context, "downloading");
  var successTitle = tr(context, "download_success");
  var id = Random().nextInt(9999);
  String? dir = await getDownloadPath();
  await client
      .read2File(file.path!, '$dir/${file.name!}${file.isDir! ? ".zip" : ""}',
          onProgress: (c, t) {
    NotificationsPlugin()
        .showProgressNotification(downloadingTitle, file.name!, id, c, t);
  });
  NotificationsPlugin()
      .showProgressNotification(successTitle, file.name!, id, 100, 100);
}

Future webDownload(String url, String fileName) async {
  final dio = Dio();
  dio.interceptors.add(InterceptorsWrapper());
  try {
    var id = Random().nextInt(9999);
    String? dir = await getDownloadPath();
    dio.download(
      url,
      '$dir/$fileName',
      onReceiveProgress: (c, t) {
        NotificationsPlugin()
            .showProgressNotification(fileName, "atrium", id, c, t);
      },
      options: Options(
          followRedirects: false,
          validateStatus: (status) {
            return status != null && status < 500;
          }),
    );
  } catch (e) {
    if (kDebugMode) {
      print(e);
    }
  }
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

openIdConnectLogin(BuildContext context) async {
  await Navigator.of(context).push(
    MaterialPageRoute<void>(
      builder: (context) {
        return const OpenIdWebView();
      },
    ),
  );
  // ignore: use_build_context_synchronously
  if (!context.mounted) return;
  if (App().hasToken) Navigator.pop(context, 'OK');
}

void redirectToAppAfterAuth() {}

class OpenIdWebView extends StatefulWidget {
  const OpenIdWebView({super.key});
  @override
  State<OpenIdWebView> createState() => _OpenIdWebViewState();
}

class _OpenIdWebViewState extends State<OpenIdWebView> {
  final cookieManager = WebviewCookieManager();
  bool _dstReached = false;
  late WebViewController controller;

  @override
  void initState() {
    super.initState();
    cookieManager.clearCookies();
    controller = WebViewController()
      ..setJavaScriptMode(JavaScriptMode.unrestricted)
      ..setNavigationDelegate(
        NavigationDelegate(
          onNavigationRequest: _interceptNavigation,
        ),
      )
      ..loadRequest(Uri.parse("${App().prefs.hostname}/auth/oauth2login"));
  }

  @override
  Widget build(BuildContext context) {
    if (!_dstReached) {
      return Scaffold(
          appBar: AppBar(
            title: const Text("Open Id Connect"),
          ),
          body: WebViewWidget(
            controller: controller,
          ));
    } else {
      cookieManager.getCookies(App().prefs.hostname).then((value) {
        var authCookie =
            value.singleWhere((element) => element.name == "ATRIUM_AUTH");
        App().cookie = "ATRIUM_AUTH=${authCookie.value}";
        Navigator.pop(context, 'OK');
      });
      return const SizedBox(
        width: 60,
        height: 60,
        child: CircularProgressIndicator(),
      );
    }
  }

  NavigationDecision _interceptNavigation(NavigationRequest request) {
    if (request.url.contains("is_admin")) {
      String xsrfToken =
          request.url.toString().split('xsrf_token=')[1].split('&')[0];
      bool isAdmin =
          request.url.toString().split('is_admin=')[1].split('&')[0] == "true";
      String username = request.url.toString().split('user=')[1].split('&')[0];
      if (xsrfToken.isNotEmpty) {
        App().isAdmin = isAdmin;
        App().xsrfToken = xsrfToken;
        App().prefs.username = username;
        setState(() {
          _dstReached = true;
        });
      }
    }
    return NavigationDecision.navigate;
  }
}

bool isWebDesktop() {
  return false;
}
