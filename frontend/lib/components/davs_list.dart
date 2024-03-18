import 'dart:math';

import 'package:atrium/components/create_edit_dav.dart';
import 'package:atrium/components/delete_dialog.dart';
import 'package:atrium/components/explorer.dart';
import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/components/sized_items_grid.dart';
import 'package:atrium/components/user_dialog.dart';
import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/sysinfo.dart';
import 'package:atrium/utils.dart';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';

import '../models/dav.dart';
import 'icons.dart';

class DavsList extends StatefulWidget {
  const DavsList({super.key});

  @override
  State<DavsList> createState() => _DavsListState();
}

class _DavsListState extends State<DavsList> {
  late Future<List<DavModel>> davs;

  @override
  void initState() {
    super.initState();
    _getData();
  }

  Future<void> openLoginDialog(_) async {
    await showLoginDialog(context, mounted);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: [
            const Icon(
              Icons.folder_open,
              size: 30,
            ),
            const SizedBox(width: 15),
            Text(tr(context, "files")),
          ],
        ),
        actions: const [UserDialogOpener()],
      ),
      body: FutureBuilder(
          future: davs,
          builder:
              (BuildContext context, AsyncSnapshot<List<DavModel>> snapshot) {
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
                    await showLoginDialog(context, mounted);
                    await _getData();
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
                      var dav = list[index];
                      var diskusage = ApiProvider().getDiskInfo(dav);
                      return Card(
                        clipBehavior: Clip.antiAlias,
                        margin: const EdgeInsets.all(8.0),
                        child: Container(
                          decoration: BoxDecoration(
                              border: Border(
                                  left:
                                      BorderSide(color: dav.color, width: 5))),
                          child: dav.isDeleting
                              ? const Center(child: DeletingSpinner())
                              : InkWell(
                                  onTap: () {
                                    _openExplorer(context, dav);
                                  },
                                  child: Column(
                                    mainAxisAlignment:
                                        MainAxisAlignment.spaceBetween,
                                    children: [
                                      Padding(
                                        padding: const EdgeInsets.only(top: 8),
                                        child: Icon(
                                          roundedIcons[dav.icon],
                                          color: dav.color,
                                          size: 70,
                                          shadows: const <Shadow>[
                                            Shadow(
                                                color: Colors.black87,
                                                blurRadius: 1.0,
                                                offset: Offset(1, 1))
                                          ],
                                        ),
                                      ),
                                      Column(
                                        children: [
                                          FutureBuilder<DiskInfo>(
                                            future: diskusage,
                                            builder: (BuildContext context,
                                                AsyncSnapshot<DiskInfo>
                                                    snapshot) {
                                              Widget child;
                                              if (snapshot.hasData) {
                                                child = Padding(
                                                  padding: const EdgeInsets
                                                      .symmetric(
                                                      horizontal: 12),
                                                  child: Column(children: [
                                                    LinearProgressIndicator(
                                                      value: snapshot
                                                          .data?.spaceUsage,
                                                      color: colorFromPercent(
                                                          snapshot.data
                                                              ?.spaceUsage),
                                                      backgroundColor:
                                                          Colors.grey[350],
                                                    ),
                                                    Text(
                                                      snapshot
                                                          .data!.usedSpaceLabel,
                                                      textAlign:
                                                          TextAlign.right,
                                                    ),
                                                  ]),
                                                );
                                              } else {
                                                child =
                                                    const SizedBox(height: 20);
                                              }
                                              return AnimatedSwitcher(
                                                duration: const Duration(
                                                    milliseconds: 250),
                                                child: child,
                                              );
                                            },
                                          ),
                                          Row(
                                            mainAxisAlignment:
                                                MainAxisAlignment.spaceBetween,
                                            crossAxisAlignment:
                                                CrossAxisAlignment.end,
                                            children: [
                                              Expanded(
                                                child: Padding(
                                                  padding: const EdgeInsets.all(
                                                      12.0),
                                                  child: Text(
                                                    dav.name,
                                                    overflow: TextOverflow.fade,
                                                    style: const TextStyle(
                                                        fontWeight:
                                                            FontWeight.bold),
                                                  ),
                                                ),
                                              ),
                                              if (App().isAdmin)
                                                PopupMenuButton(
                                                    itemBuilder:
                                                        (BuildContext
                                                                context) =>
                                                            <PopupMenuEntry>[
                                                              PopupMenuItem(
                                                                  onTap: () {
                                                                    WidgetsBinding
                                                                        .instance
                                                                        .addPostFrameCallback(
                                                                            (_) async {
                                                                      await Navigator.push(
                                                                          context,
                                                                          MaterialPageRoute(
                                                                            builder: (context) =>
                                                                                CreateEditDav(dav: dav, isNew: false),
                                                                          ));
                                                                      await _getData();
                                                                      setState(
                                                                          () {});
                                                                    });
                                                                  },
                                                                  child: Row(
                                                                    children: [
                                                                      const Padding(
                                                                        padding:
                                                                            EdgeInsets.all(8.0),
                                                                        child: Icon(
                                                                            Icons.edit),
                                                                      ),
                                                                      Text(tr(
                                                                          context,
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
                                                                        builder:
                                                                            (context) =>
                                                                                DeleteDialog(dav.name),
                                                                      );
                                                                      if (confirmed!) {
                                                                        setState(
                                                                            () {
                                                                          dav.isDeleting =
                                                                              true;
                                                                        });
                                                                        await ApiProvider()
                                                                            .deleteDav(dav.id);
                                                                        await _getData();
                                                                        setState(
                                                                            () {});
                                                                      }
                                                                    });
                                                                  },
                                                                  child: Row(
                                                                    children: [
                                                                      const Padding(
                                                                        padding:
                                                                            EdgeInsets.all(8.0),
                                                                        child: Icon(
                                                                            Icons.delete_forever),
                                                                      ),
                                                                      Text(tr(
                                                                          context,
                                                                          "delete"))
                                                                    ],
                                                                  )),
                                                            ])
                                            ],
                                          )
                                        ],
                                      ),
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
                var davs = await ApiProvider().getDavs();
                var maxId =
                    davs.isNotEmpty ? davs.map((e) => e.id).reduce(max) : 0;
                var dav = DavModel(id: maxId + 1);
                if (!context.mounted) return;
                await Navigator.push(
                    context,
                    MaterialPageRoute(
                      builder: (context) =>
                          CreateEditDav(dav: dav, isNew: true),
                    ));
                await _getData();
                setState(() {});
              })
          : null,
    );
  }

  _getData() {
    davs = App().isAdmin ? ApiProvider().getDavs() : ApiProvider().listDavs();
  }
}

void _openExplorer(BuildContext context, DavModel dav) {
  Navigator.of(context).push(
    MaterialPageRoute<void>(
      builder: (context) {
        return Explorer(
          dav: dav,
        );
      },
    ),
  );
}
