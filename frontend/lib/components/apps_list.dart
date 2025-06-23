import 'dart:math';

import 'package:atrium/components/create_edit_app.dart';
import 'package:atrium/components/delete_dialog.dart';
import 'package:atrium/components/sized_items_grid.dart';

import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/components/webview.dart'
    if (dart.library.html) 'package:atrium/components/iframe_webview.dart';
import 'package:atrium/components/user_dialog.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher_string.dart';

import '../globals.dart';
import '../models/app.dart';
import 'icons.dart';

class AppsList extends StatefulWidget {
  const AppsList({super.key});

  @override
  State<AppsList> createState() => _AppsListState();
}

class _AppsListState extends State<AppsList> {
  Future<void> openLoginDialog(dynamic _) async {
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
        title: Row(
          children: [
            const Icon(
              Icons.apps,
              size: 30,
            ),
            const SizedBox(width: 15),
            Text(tr(context, "apps")),
          ],
        ),
        actions: const [UserDialogOpener()],
      ),
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
                    snapshot.error is DioException &&
                    (snapshot.error as DioException).response?.statusCode ==
                        401) {
                  // If error is 401, we log and retry
                  Future.delayed(Duration.zero, () async {
                    if (context.mounted) {
                      await showLoginDialog(context, mounted);
                    }
                    _getData();
                    setState(() {});
                  });
                  return const Center(child: CircularProgressIndicator());
                }
                if (snapshot.hasError) {
                  return Center(child: Text('Error: ${snapshot.error}'));
                }
                var list = snapshot.data ?? [];
                return SizedItemsGrid(
                    itemBuilder: (context, index) {
                      var app = list[index];
                      return Card(
                        margin: const EdgeInsets.all(8.0),
                        clipBehavior: Clip.antiAlias,
                        child: Container(
                          decoration: BoxDecoration(
                              border: Border(
                                  left:
                                      BorderSide(color: app.color, width: 5))),
                          child: app.isDeleting
                              ? const Center(child: DeletingSpinner())
                              : InkWell(
                                  onTap: () {
                                    _openAppInWebView(context, app);
                                  },
                                  child: Column(
                                    mainAxisAlignment:
                                        MainAxisAlignment.spaceBetween,
                                    children: [
                                      Padding(
                                        padding: const EdgeInsets.all(12.0),
                                        child: Icon(
                                          roundedIcons[app.icon],
                                          color: app.color,
                                          size: 70,
                                          shadows: const <Shadow>[
                                            Shadow(
                                                color: Colors.black87,
                                                blurRadius: 1.0,
                                                offset: Offset(1, 1))
                                          ],
                                        ),
                                      ),
                                      Row(
                                        mainAxisAlignment:
                                            MainAxisAlignment.spaceBetween,
                                        crossAxisAlignment:
                                            CrossAxisAlignment.end,
                                        children: [
                                          Expanded(
                                            child: Padding(
                                              padding:
                                                  const EdgeInsets.all(12.0),
                                              child: Text(
                                                app.name,
                                                overflow: TextOverflow.fade,
                                                style: const TextStyle(
                                                    fontWeight:
                                                        FontWeight.bold),
                                              ),
                                            ),
                                          ),
                                          PopupMenuButton(
                                              itemBuilder: (BuildContext
                                                      context) =>
                                                  <PopupMenuEntry>[
                                                    PopupMenuItem(
                                                        onTap: () {
                                                          launchUrlString(
                                                              modelUrl(app));
                                                        },
                                                        child: Row(
                                                          children: [
                                                            const Padding(
                                                              padding:
                                                                  EdgeInsets
                                                                      .all(8.0),
                                                              child: Icon(
                                                                  Icons.tab),
                                                            ),
                                                            Text(tr(context,
                                                                "open_in_new_tab"))
                                                          ],
                                                        )),
                                                    if (App().isAdmin) ...[
                                                      PopupMenuItem(
                                                          onTap: () {
                                                            WidgetsBinding
                                                                .instance
                                                                .addPostFrameCallback(
                                                                    (_) async {
                                                              await Navigator.push(
                                                                  context,
                                                                  MaterialPageRoute(
                                                                    builder: (context) => CreateEditApp(
                                                                        app:
                                                                            app,
                                                                        isNew:
                                                                            false),
                                                                  ));
                                                              _getData();
                                                              setState(() {});
                                                            });
                                                          },
                                                          child: Row(
                                                            children: [
                                                              const Padding(
                                                                padding:
                                                                    EdgeInsets
                                                                        .all(
                                                                            8.0),
                                                                child: Icon(
                                                                    Icons.edit),
                                                              ),
                                                              Text(tr(context,
                                                                  "edit"))
                                                            ],
                                                          )),
                                                      PopupMenuItem(
                                                          onTap: () {
                                                            WidgetsBinding
                                                                .instance
                                                                .addPostFrameCallback(
                                                                    (_) async {
                                                              var confirmed =
                                                                  await showDialog<
                                                                      bool>(
                                                                context:
                                                                    context,
                                                                builder: (context) =>
                                                                    DeleteDialog(
                                                                        app.name),
                                                              );
                                                              if (confirmed!) {
                                                                setState(() {
                                                                  app.isDeleting =
                                                                      true;
                                                                });
                                                                await ApiProvider()
                                                                    .deleteApp(
                                                                        app.id);
                                                                _getData();
                                                                setState(() {});
                                                              }
                                                            });
                                                          },
                                                          child: Row(
                                                            children: [
                                                              const Padding(
                                                                padding:
                                                                    EdgeInsets
                                                                        .all(
                                                                            8.0),
                                                                child: Icon(Icons
                                                                    .delete_forever),
                                                              ),
                                                              Text(tr(context,
                                                                  "delete"))
                                                            ],
                                                          ))
                                                    ],
                                                  ])
                                        ],
                                      )
                                    ],
                                  ),
                                ),
                        ),
                      );
                    },
                    list: list);
            }
          }),
      floatingActionButton: App().isAdmin
          ? FloatingActionButton.small(
              child: const Icon(Icons.add),
              onPressed: () async {
                var apps = await ApiProvider().getApps();
                var maxId =
                    apps.isNotEmpty ? apps.map((e) => e.id).reduce(max) : 0;
                var app = AppModel(id: maxId + 1);
                if (!context.mounted) return;
                await Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (context) =>
                          CreateEditApp(app: app, isNew: true),
                    ));
                _getData();
                setState(() {});
              })
          : null,
    );
  }

  Null _getData() {
    apps = App().isAdmin ? ApiProvider().getApps() : ApiProvider().listApps();
  }
}

void _openAppInWebView(BuildContext context, AppModel app) {
  Navigator.of(context).push(
    MaterialPageRoute<void>(
      builder: (context) {
        var initialUrl = modelUrl(app) + app.openpath;
        return Scaffold(
            appBar: AppBar(
              backgroundColor: app.color,
              title: Row(
                children: [
                  Icon(
                    roundedIcons[app.icon],
                    size: 30,
                  ),
                  const SizedBox(width: 15),
                  Text(app.name),
                ],
              ),
            ),
            body: AppWebView(
              initialUrl: initialUrl,
            ));
      },
    ),
  );
}
