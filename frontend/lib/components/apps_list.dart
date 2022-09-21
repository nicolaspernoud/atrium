import 'dart:math';

import 'package:atrium/components/create_edit_app.dart';

import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/components/webview.dart'
    if (dart.library.html) 'package:atrium/components/iframe_webview.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/utils.dart';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';

import '../globals.dart';
import '../models/app.dart';

class AppsList extends StatefulWidget {
  const AppsList({Key? key}) : super(key: key);

  @override
  State<AppsList> createState() => _AppsListState();
}

class _AppsListState extends State<AppsList> {
  Future<void> openLoginDialog(_) async {
    await showLoginDialog(context, mounted);
  }

  late Future<List<AppModel>> apps;

  @override
  void initState() {
    super.initState();
    _getData();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(tr(context, "apps")),
        actions: logoutAction,
      ),
      body: Padding(
        padding: const EdgeInsets.all(8.0),
        child: FutureBuilder(
            future: apps,
            builder:
                (BuildContext context, AsyncSnapshot<List<AppModel>> snapshot) {
              switch (snapshot.connectionState) {
                case ConnectionState.none:
                case ConnectionState.active:
                case ConnectionState.waiting:
                  return const Center(child: CircularProgressIndicator());
                case ConnectionState.done:
                  if (snapshot.hasError &&
                      snapshot.error is DioError &&
                      (snapshot.error as DioError).response?.statusCode ==
                          401) {
                    // If error is 401, we log and retry
                    Future.delayed(Duration.zero, () async {
                      await showLoginDialog(context, mounted);
                      await _getData();
                      setState(() {});
                    });
                    return const Center(child: CircularProgressIndicator());
                  }
                  if (snapshot.hasError) {
                    return Center(child: Text('Error: ${snapshot.error}'));
                  }
                  return _buildListView(context, snapshot.data ?? []);
              }
            }),
      ),
      floatingActionButton: App().isAdmin
          ? FloatingActionButton(
              child: const Icon(Icons.add),
              onPressed: () async {
                var apps = await ApiProvider().getApps();
                var maxId = apps.map((e) => e.id).reduce(max);
                var app = AppModel(id: maxId + 1);
                if (!mounted) return;
                await Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (context) =>
                          CreateEditApp(app: app, isNew: true),
                    ));
                await _getData();
                setState(() {});
              })
          : null,
    );
  }

  _getData() {
    apps = App().isAdmin ? ApiProvider().getApps() : ApiProvider().listApps();
  }

  Widget _buildListView(BuildContext context, List<AppModel> list) {
    return Wrap(
        children: list
            .map((app) => Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: Card(
                    child: ListTile(
                      leading: Icon(
                        IconData(app.icon, fontFamily: 'MaterialIcons'),
                        color: app.color,
                        size: 50,
                        shadows: const <Shadow>[
                          Shadow(
                              color: Colors.black87,
                              blurRadius: 1.0,
                              offset: Offset(1, 1))
                        ],
                      ),
                      title: Padding(
                        padding: const EdgeInsets.all(20.0),
                        child: Text(
                          app.name,
                          style: const TextStyle(fontWeight: FontWeight.bold),
                        ),
                      ),
                      onTap: () {
                        _openAppInWebView(context, app);
                      },
                      trailing: App().isAdmin
                          ? PopupMenuButton(
                              itemBuilder: (BuildContext context) =>
                                  <PopupMenuEntry>[
                                    PopupMenuItem(
                                        onTap: () {
                                          WidgetsBinding.instance
                                              .addPostFrameCallback((_) async {
                                            await Navigator.push(
                                                context,
                                                MaterialPageRoute(
                                                  builder: (context) =>
                                                      CreateEditApp(
                                                          app: app,
                                                          isNew: false),
                                                ));
                                            await _getData();
                                            setState(() {});
                                          });
                                        },
                                        child: Row(
                                          children: [
                                            const Padding(
                                              padding: EdgeInsets.all(8.0),
                                              child: Icon(Icons.edit),
                                            ),
                                            Text(tr(context, "edit"))
                                          ],
                                        )),
                                    PopupMenuItem(
                                        onTap: () {
                                          WidgetsBinding.instance
                                              .addPostFrameCallback((_) async {
                                            await ApiProvider()
                                                .deleteApp(app.id);
                                            await _getData();
                                            setState(() {});
                                          });
                                        },
                                        child: Row(
                                          children: [
                                            const Padding(
                                              padding: EdgeInsets.all(8.0),
                                              child: Icon(Icons.delete_forever),
                                            ),
                                            Text(tr(context, "delete"))
                                          ],
                                        )),
                                  ])
                          : null,
                    ),
                  ),
                ))
            .toList());
  }
}

void _openAppInWebView(BuildContext context, AppModel app) {
  Navigator.of(context).push(
    MaterialPageRoute<void>(
      builder: (context) {
        var parts = App().prefs.hostname.split("://");
        var initialUrl = "${parts[0]}://${app.host}.${parts[1]}";
        return Scaffold(
            appBar: AppBar(
              title: Text(app.name),
            ),
            body: AppWebView(
              initialUrl: initialUrl,
            ));
      },
    ),
  );
}
