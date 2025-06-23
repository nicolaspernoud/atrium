import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:dio/dio.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:webdav_client/webdav_client.dart';
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';

class UploadsList extends StatefulWidget {
  const UploadsList({super.key});
  @override
  State<UploadsList> createState() => _UploadsListState();
}

class _UploadsListState extends State<UploadsList> {
  @override
  Widget build(BuildContext context) {
    return Consumer<App>(builder: (context, app, child) {
      return Padding(
        padding: const EdgeInsets.all(16.0),
        child: Column(
          children: [
            Text(tr(context, "uploads")),
            Expanded(
              child: ListView(padding: const EdgeInsets.all(8), children: [
                ...app.uploads.map((e) => ListTile(
                    title: Text(e.file.name),
                    subtitle: LinearProgressIndicator(value: e.progress),
                    trailing: (e.status == Status.finished)
                        ? const Icon(
                            Icons.done,
                            color: Colors.green,
                          )
                        : (e.status == Status.pending ||
                                e.status == Status.uploading)
                            ? IconButton(
                                icon: const Icon(Icons.close),
                                onPressed: () {
                                  e.cancelToken.cancel();
                                  e.status = Status.error;
                                  app.reportProgress();
                                })
                            : PopupMenuButton(
                                itemBuilder: (BuildContext context) =>
                                    <PopupMenuEntry>[
                                  if (!kIsWeb)
                                    PopupMenuItem(
                                        onTap: () async {
                                          await e.doUpload();
                                        },
                                        child: Row(
                                          children: [
                                            const Padding(
                                              padding: EdgeInsets.all(8.0),
                                              child: Icon(Icons.replay,
                                                  color: Colors.orange),
                                            ),
                                            Text(tr(context, "retry"))
                                          ],
                                        )),
                                  PopupMenuItem(
                                      onTap: () {
                                        e.cancelToken.cancel();
                                        app.uploads.remove(e);
                                        app.reportProgress();
                                      },
                                      child: Row(
                                        children: [
                                          const Padding(
                                            padding: EdgeInsets.all(8.0),
                                            child: Icon(Icons.close,
                                                color: Colors.red),
                                          ),
                                          Text(tr(context, "cancel"))
                                        ],
                                      ))
                                ],
                              )))
              ]),
            ),
            InkWell(
              onTap: () {
                var toRemove = [];
                for (var e in app.uploads) {
                  if (e.status == Status.finished) {
                    toRemove.add(e);
                  }
                }
                app.uploads.removeWhere((e) => toRemove.contains(e));
                app.reportProgress();
                if (app.uploads.isEmpty) {
                  Navigator.pop(context);
                }
              },
              child: Row(
                children: [
                  const Padding(
                    padding: EdgeInsets.all(8.0),
                    child: Icon(Icons.done_all),
                  ),
                  Text(tr(context, "remove_dones"))
                ],
              ),
            )
          ],
        ),
      );
    });
  }
}

class Upload {
  PlatformFile file;
  Client client;
  String destPath;
  late CancelToken cancelToken;
  Status status = Status.pending;
  DateTime addTime = DateTime.now();
  double progress = 0.0;

  Upload(this.client, this.file, this.destPath);

  Future<void> doUpload() async {
    cancelToken = CancelToken();
    status = Status.uploading;
    await upload(destPath, file, client, onProgress, cancelToken);
    status = Status.finished;
    progress = 1.0;
    App().reportProgress();
  }

  void onProgress(int c, int t) {
    progress = c / t;
    App().reportProgress();
  }
}

class Uploads {
  List<Upload> uploads = [];

  Stream<Upload?> uploadAll() async* {
    uploads.sort((a, b) => a.addTime.compareTo(b.addTime));
    for (var u in uploads) {
      if (u.status == Status.pending) {
        try {
          await u.doUpload();
          yield u;
        } catch (e) {
          u.status = Status.error;
          yield null;
        }
      }
    }
  }

  void push(
    Client client,
    PlatformFile file,
    String dir,
  ) {
    uploads.add(Upload(client, file, dir));
  }
}

enum Status { pending, uploading, error, finished }
