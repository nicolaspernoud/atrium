import 'dart:math';

import 'package:atrium/components/create_edit_dav.dart';
import 'package:atrium/components/explorer.dart';
import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/sysinfo.dart';
import 'package:atrium/utils.dart';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';

import '../models/dav.dart';

class DavsList extends StatefulWidget {
  const DavsList({Key? key}) : super(key: key);

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
      appBar: AppBar(title: Text(tr(context, "davs_list"))),
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
                var davs = await ApiProvider().getDavs();
                var maxId = davs.map((e) => e.id).reduce(max);
                var dav = DavModel(id: maxId + 1);
                if (!mounted) return;
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

  Widget _buildListView(BuildContext context, List<DavModel> list) {
    return ListView.builder(
        itemCount: list.length,
        itemBuilder: (context, index) {
          final dav = list[index];
          var diskusage = ApiProvider().getDiskInfo(dav);
          return ListTile(
              leading: Icon(IconData(dav.icon, fontFamily: 'MaterialIcons'),
                  color: dav.color),
              title: Text(dav.name),
              subtitle: FutureBuilder<DiskInfo>(
                future: diskusage,
                builder:
                    (BuildContext context, AsyncSnapshot<DiskInfo> snapshot) {
                  if (snapshot.hasData) {
                    return Row(
                      children: [
                        Expanded(
                          child: LinearProgressIndicator(
                            value: snapshot.data?.spaceUsage,
                            color: colorFromPercent(snapshot.data?.spaceUsage),
                            backgroundColor: Colors.grey[350],
                          ),
                        ),
                        Padding(
                          padding: const EdgeInsets.only(left: 8.0),
                          child: Text(snapshot.data!.usedSpaceLabel),
                        )
                      ],
                    );
                  } else {
                    return Container();
                  }
                },
              ),
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
                                        CreateEditDav(dav: dav, isNew: false),
                                  ));
                              await _getData();
                              setState(() {});
                            }),
                        IconButton(
                            icon: const Icon(Icons.delete_forever),
                            onPressed: () async {
                              await ApiProvider().deleteDav(dav.id);
                              await _getData();
                              setState(() {});
                            }),
                      ],
                    )
                  : null,
              onTap: () {
                _openExplorer(context, dav);
              });
        });
  }
}

void _openExplorer(BuildContext context, DavModel dav) {
  Navigator.of(context).push(
    MaterialPageRoute<void>(
      builder: (context) {
        String url = davUrl(dav);
        return Explorer(
          url: url,
          name: dav.name,
          readWrite: dav.writable,
        );
      },
    ),
  );
}
