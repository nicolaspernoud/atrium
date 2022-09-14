import 'dart:math';

import 'package:atrium/components/create_edit_app.dart';

import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/components/webview.dart'
    if (dart.library.html) 'package:atrium/components/iframe_webview.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
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
      appBar: AppBar(title: Text(tr(context, "apps_list"))),
      body: FutureBuilder(
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
                    (snapshot.error as DioError).response?.statusCode == 401) {
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
    return ListView.builder(
        itemCount: list.length,
        itemBuilder: (context, index) {
          final app = list[index];
          return ListTile(
            leading: Icon(IconData(app.icon, fontFamily: 'MaterialIcons'),
                color: app.color),
            title: Text(app.name),
            subtitle: const Text("subtitle"),
            onTap: () {
              _openAppInWebView(context, app);
            },
            trailing: App().isAdmin
                ? Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      IconButton(
                          icon: const Icon(Icons.edit),
                          onPressed: () async {
                            await Navigator.push(
                                context,
                                MaterialPageRoute(
                                  builder: (context) =>
                                      CreateEditApp(app: app, isNew: false),
                                ));
                            await _getData();
                            setState(() {});
                          }),
                      IconButton(
                          icon: const Icon(Icons.delete_forever),
                          onPressed: () async {
                            await ApiProvider().deleteApp(app.id);
                            await _getData();
                            setState(() {});
                          }),
                    ],
                  )
                : null,
          );
        });
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
