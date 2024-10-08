import 'dart:async';

import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/components/user_dialog.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/sysinfo.dart';
import 'package:atrium/utils.dart';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';

class SystemInfo extends StatefulWidget {
  const SystemInfo({super.key});

  @override
  State<SystemInfo> createState() => _SystemInfoState();
}

class _SystemInfoState extends State<SystemInfo> {
  Future<void> openLoginDialog(_) async {
    await showLoginDialog(context, mounted);
  }

  late Future<SysInfo> sysInfo;

  @override
  void initState() {
    super.initState();
    _getData();
    Timer.periodic(const Duration(seconds: 1), (Timer t) {
      if (!mounted) return;
      setState(() {
        _getData();
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: [
            const Icon(
              Icons.monitor_heart,
              size: 30,
            ),
            const SizedBox(width: 15),
            Text(tr(context, "system_information")),
          ],
        ),
        actions: const [UserDialogOpener()],
      ),
      body: Padding(
        padding: const EdgeInsets.all(8.0),
        child: FutureBuilder(
            future: sysInfo,
            builder: (BuildContext context, AsyncSnapshot<SysInfo> snapshot) {
              if (snapshot.hasError &&
                  snapshot.error is DioException &&
                  (snapshot.error as DioException).response?.statusCode ==
                      401) {
                // If error is 401, we log and retry
                Future.delayed(Duration.zero, () async {
                  if (context.mounted) await showLoginDialog(context, mounted);
                  await _getData();
                  setState(() {});
                });
                return const Center(child: CircularProgressIndicator());
              }
              if (snapshot.hasError) {
                return Center(child: Text('Error: ${snapshot.error}'));
              }
              if (!snapshot.hasData) {
                return Container();
              }
              return Column(children: [
                Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: Card(
                    child: Padding(
                      padding: const EdgeInsets.all(10.0),
                      child: ListTile(
                        leading: const Icon(Icons.computer),
                        title: Text(tr(context, "cpu_usage")),
                        subtitle: LinearProgressIndicator(
                          value: snapshot.data!.cpuUsage,
                          color: colorFromPercent(snapshot.data!.cpuUsage),
                          backgroundColor: Colors.grey[350],
                        ),
                      ),
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: Card(
                    child: Padding(
                      padding: const EdgeInsets.all(10.0),
                      child: ListTile(
                        leading: const Icon(Icons.memory),
                        title: Text(tr(context, "memory_usage")),
                        subtitle: LinearProgressIndicator(
                          value: snapshot.data!.memoryUsage,
                          color: colorFromPercent(snapshot.data!.memoryUsage),
                          backgroundColor: Colors.grey[350],
                        ),
                      ),
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: Card(
                    child: Padding(
                      padding: const EdgeInsets.all(10.0),
                      child: ListTile(
                          leading: const Icon(Icons.timer),
                          title: Text(tr(context, "uptime")),
                          subtitle: Text(MyLocalizations.of(context)!
                              .formatDuration(
                                  Duration(seconds: snapshot.data!.uptime)))),
                    ),
                  ),
                ),
              ]);
            }),
      ),
    );
  }

  _getData() {
    sysInfo = ApiProvider().getSysInfo();
  }
}
