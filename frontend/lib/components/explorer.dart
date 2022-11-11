import 'package:atrium/components/delete_dialog.dart';
import 'package:atrium/components/icons.dart';
import 'package:atrium/components/image_viewer.dart';
import 'package:atrium/components/media_player.dart';
import 'package:atrium/components/pdf_viewer.dart';
import 'package:atrium/components/rename_dialog.dart';
import 'package:atrium/components/share_dialog.dart';
import 'package:atrium/components/text_editor.dart';
import 'package:atrium/components/uploads.dart';
import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/dav.dart';
import 'package:dio/dio.dart';
import 'package:file_picker/file_picker.dart';
import 'package:filesize/filesize.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:mime/mime.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:webdav_client/webdav_client.dart' as webdav;
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';
import 'package:webdav_client/webdav_client.dart';

class Explorer extends StatefulWidget {
  late final String url;
  late final DavModel dav;
  final bool readWrite;
  // ignore: prefer_const_constructors_in_immutables
  Explorer({Key? key, required this.dav})
      : url = modelUrl(dav),
        readWrite = dav.writable,
        super(key: key);

  @override
  ExplorerState createState() => ExplorerState();
}

enum CopyMoveStatus { none, copy, move }

class ExplorerState extends State<Explorer> {
  late webdav.Client client;
  final user = 'dummy';
  final pwd = App().token;
  var dirPath = '/';
  var _copyMoveStatus = CopyMoveStatus.none;
  var _copyMovePath = "";
  late bool readWrite;
  late Future<List<File>> files;

  @override
  void initState() {
    super.initState();
    readWrite = widget.readWrite;
    // init client
    client = newExplorerClient(
      widget.url,
      user: user,
      password: pwd,
      debug: false,
    );
    _getData();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.url.isEmpty || user.isEmpty || pwd.isEmpty) {
      return const Center(child: Text("you need add url || user || pwd"));
    }
    return Scaffold(
      appBar: AppBar(
        backgroundColor: widget.dav.color,
        title: Row(
          children: [
            Icon(
              roundedIcons[widget.dav.icon],
              size: 30,
            ),
            const SizedBox(width: 15),
            Text(widget.dav.name),
          ],
        ),
      ),
      body: FutureBuilder(
          future: files,
          builder: (BuildContext context,
              AsyncSnapshot<List<webdav.File>> snapshot) {
            switch (snapshot.connectionState) {
              case ConnectionState.none:
              case ConnectionState.active:
              case ConnectionState.waiting:
                return const Center(child: CircularProgressIndicator());
              case ConnectionState.done:
                if (snapshot.hasError) {
                  return Center(child: Text('Error: ${snapshot.error}'));
                }
                return _buildListView(context, snapshot.data ?? []);
            }
          }),
      bottomNavigationBar: BottomAppBar(
          child: Row(children: [
        IconButton(
            icon: const Icon(Icons.home),
            onPressed: () {
              dirPath = "/";
              setState(() {
                _getData();
              });
            }),
        if (readWrite)
          IconButton(
              icon: const Icon(Icons.create_new_folder),
              onPressed: () async {
                CancelToken c = CancelToken();
                await client.mkdir("$dirPath/newfolder", c);
                setState(() {
                  _getData();
                });
              }),
        if (readWrite)
          IconButton(
              icon: const Icon(Icons.add),
              onPressed: () async {
                CancelToken c = CancelToken();
                await client.write("$dirPath/newfile.txt", Uint8List(0),
                    onProgress: (c, t) {
                  debugPrint((c / t).toString());
                }, cancelToken: c);
                setState(() {
                  _getData();
                });
              }),
        if (readWrite)
          Consumer<App>(builder: (context, app, child) {
            return app.hasUploads
                ? IconButton(
                    icon: const Icon(Icons.playlist_add_check),
                    onPressed: () {
                      showModalBottomSheet<void>(
                        context: context,
                        builder: (BuildContext context) {
                          return const UploadsList();
                        },
                      );
                    },
                  )
                : IconButton(
                    icon: const Icon(Icons.upload),
                    onPressed: () async {
                      FilePickerResult? result =
                          await FilePicker.platform.pickFiles(
                        allowMultiple: true,
                        withReadStream: true,
                      );
                      if (result != null) {
                        for (var file in result.files) {
                          app.pushUpload(client, file, dirPath);
                        }
                        while (app.uploads
                            .where(
                                (element) => element.status == Status.pending)
                            .isNotEmpty) {
                          var currentUpload = await app.uploadOne();
                          // We refresh the view only if we are still in the same directory
                          if (currentUpload != null &&
                              dirPath == currentUpload.destPath) {
                            setState(() {
                              _getData();
                            });
                          }
                        }
                      }
                    });
          }),
        if (_copyMoveStatus != CopyMoveStatus.none)
          IconButton(
              icon: const Icon(Icons.paste),
              onPressed: () async {
                CancelToken c = CancelToken();
                String dest;
                // Case of directory
                if (_copyMovePath.endsWith("/")) {
                  dest = dirPath +
                      _copyMovePath
                          .substring(0, _copyMovePath.length - 1)
                          .split("/")
                          .last;
                } else {
                  dest = dirPath;
                }
                if (_copyMoveStatus == CopyMoveStatus.copy) {
                  await client.copy(_copyMovePath, dest, true, c);
                } else {
                  await client.rename(_copyMovePath, dest, true, c);
                }
                setState(() {
                  _copyMoveStatus = CopyMoveStatus.none;
                  _getData();
                });
              })
      ])),
    );
  }

  void _getData() {
    files = client.readDir(dirPath);
  }

  Widget _buildListView(BuildContext context, List<webdav.File> list) {
    // Sort : folders first and then alphabetically
    list.sort((a, b) {
      if (a.isDir! && !(b.isDir!)) {
        return -1;
      }
      if (!(a.isDir!) && (b.isDir!)) {
        return 1;
      }
      if (a.name != null && b.name != null) {
        return a.name!.compareTo(b.name!);
      }
      return 0;
    });
    final List idxList = Iterable<int>.generate(list.length).toList();
    return ListView(children: [
      if (dirPath != "/")
        ListTile(
          leading: const Icon(Icons.reply),
          title: const Text(".."),
          onTap: () {
            dirPath = dirPath.substring(0, dirPath.length - 1);
            dirPath = dirPath.substring(0, dirPath.lastIndexOf("/") + 1);
            setState(() {
              _getData();
            });
          },
        ),
      ...idxList.map((idx) {
        var file = list[idx];
        var mimeType = lookupMimeType(file.name!);
        return ListTile(
          leading: widgetFromFileType(file, mimeType),
          title: Text(file.name ?? ''),
          subtitle: Text(formatTime(file.mTime) +
              ((file.size != null && file.size! > 0)
                  ? " - ${filesize(file.size, 0)}"
                  : "")),
          trailing: PopupMenuButton(
              itemBuilder: (BuildContext context) => <PopupMenuEntry>[
                    PopupMenuItem(
                        onTap: () =>
                            download(widget.url, client, file, context),
                        child: Row(
                          children: [
                            const Padding(
                              padding: EdgeInsets.all(8.0),
                              child: Icon(Icons.download),
                            ),
                            Text(tr(context, "download"))
                          ],
                        )),
                    PopupMenuItem(
                        onTap: () {
                          WidgetsBinding.instance
                              .addPostFrameCallback((_) async {
                            await showDialog(
                                context: context,
                                builder: (context) =>
                                    ShareDialog(widget.url, file));
                          });
                        },
                        child: Row(
                          children: [
                            const Padding(
                              padding: EdgeInsets.all(8.0),
                              child: Icon(Icons.share),
                            ),
                            Text(tr(context, "share"))
                          ],
                        )),
                    if (readWrite) ...[
                      PopupMenuItem(
                          onTap: () {
                            WidgetsBinding.instance
                                .addPostFrameCallback((_) async {
                              String? val = await showDialog<String>(
                                context: context,
                                builder: (context) => RenameDialog(file.name!),
                              );
                              if (val != null && file.path != null) {
                                var newPath = file.path!;
                                newPath = newPath.endsWith("/")
                                    ? newPath.substring(0, newPath.length - 1)
                                    : newPath;
                                newPath =
                                    "${newPath.substring(0, newPath.lastIndexOf('/'))}/$val";
                                newPath = file.isDir! ? "$newPath/" : newPath;
                                await client.rename(file.path!, newPath, true);
                                setState(() {
                                  _getData();
                                });
                              }
                            });
                          },
                          child: Row(
                            children: [
                              const Padding(
                                padding: EdgeInsets.all(8.0),
                                child: Icon(Icons.drive_file_rename_outline),
                              ),
                              Text(tr(context, "rename"))
                            ],
                          )),
                      PopupMenuItem(
                          onTap: (() {
                            setState(() {
                              _copyMoveStatus = CopyMoveStatus.copy;
                              _copyMovePath = file.path!;
                            });
                          }),
                          child: Row(
                            children: [
                              Padding(
                                padding: const EdgeInsets.all(8.0),
                                child: Icon(Icons.copy,
                                    color: _copyMovePath == file.path! &&
                                            _copyMoveStatus ==
                                                CopyMoveStatus.copy
                                        ? Colors.blueAccent
                                        : null),
                              ),
                              Text(tr(context, "copy"))
                            ],
                          )),
                      PopupMenuItem(
                          onTap: (() {
                            setState(() {
                              _copyMoveStatus = CopyMoveStatus.move;
                              _copyMovePath = file.path!;
                            });
                          }),
                          child: Row(
                            children: [
                              Padding(
                                padding: const EdgeInsets.all(8.0),
                                child: Icon(Icons.cut,
                                    color: _copyMovePath == file.path! &&
                                            _copyMoveStatus ==
                                                CopyMoveStatus.move
                                        ? Colors.blueAccent
                                        : null),
                              ),
                              Text(tr(context, "cut"))
                            ],
                          )),
                      PopupMenuItem(
                          onTap: () async {
                            WidgetsBinding.instance
                                .addPostFrameCallback((_) async {
                              var confirmed = await showDialog<bool>(
                                context: context,
                                builder: (context) => DeleteDialog(file.name!),
                              );
                              if (confirmed!) {
                                await client.removeAll(file.path!);
                                setState(() {
                                  idxList.removeAt(idx);
                                  list.removeAt(idx);
                                });
                              }
                            });
                          },
                          child: Row(
                            children: [
                              const Padding(
                                padding: EdgeInsets.all(8.0),
                                child: Icon(Icons.delete),
                              ),
                              Text(tr(context, "delete"))
                            ],
                          ))
                    ]
                  ]),
          onTap: () async {
            if (file.isDir!) {
              dirPath = file.path!;
              setState(() {
                _getData();
              });
            } else {
              if (mimeType != null) {
                if (mimeType.contains("text/") ||
                    mimeType.contains("json") ||
                    mimeType.contains("x-sh")) {
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                        builder: (context) => TextEditor(
                            client: client, file: file, readWrite: readWrite)),
                  );
                } else if (mimeType.contains("image")) {
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                        builder: (context) => ImageViewer(
                              client: client,
                              url: widget.url,
                              files: list,
                              index: idx,
                            )),
                  );
                } else if (mimeType.contains("pdf")) {
                  Navigator.push(
                    context,
                    MaterialPageRoute(
                        builder: (context) => PdfViewer(
                            client: client, url: widget.url, file: file)),
                  );
                } else if (mimeType.contains("openxmlformats") ||
                    mimeType.contains("opendocument")) {
                  // Get a share token for this document
                  var shareToken = await ApiProvider().getShareToken(
                      widget.url.split("://")[1].split(":")[0], file.path!,
                      shareWith: "external_editor", shareForDays: 1);
                  final Uri launchUri = Uri(
                    scheme: App().prefs.hostnameScheme,
                    host: App().prefs.hostnameHost,
                    port: App().prefs.hostnamePort,
                    path: 'onlyoffice',
                    query: joinQueryParameters(<String, String>{
                      'file': '${widget.url}${file.path}',
                      'mtime': file.mTime!.toIso8601String(),
                      'user': App().prefs.username,
                      'token': shareToken!
                    }),
                  );
                  launchUrl(launchUri);
                } else if (mimeType.contains("video/") ||
                    mimeType.contains("audio/")) {
                  String uri = '${widget.url}${escapePath(file.path!)}';
                  if (kIsWeb) {
                    var shareToken = await ApiProvider().getShareToken(
                        widget.url.split("://")[1].split(":")[0], file.path!,
                        shareWith: "media_player", shareForDays: 1);
                    uri = '$uri?token=$shareToken';
                  }
                  if (!mounted) return;
                  Navigator.push(
                      context,
                      MaterialPageRoute(
                          builder: (context) =>
                              MediaPlayer(uri: uri, file: file)));
                }
              }
            }
          },
        );
      })
    ]);
  }

  Widget widgetFromFileType(File file, String? mimeType) {
    if (file.isDir != null && file.isDir!) {
      return const Icon(Icons.folder, size: 30);
    }
    if (mimeType != null && mimeType.contains("image")) {
      return SizedBox(
        width: 30,
        height: 30,
        child: FutureBuilder<Uint8List>(
            future: client
                .read(file.path!)
                .then((value) => Uint8List.fromList(value)),
            builder: (BuildContext context, AsyncSnapshot<Uint8List> snapshot) {
              Widget child;
              if (snapshot.hasData) {
                child = Image.memory(snapshot.data!);
              } else if (snapshot.hasError) {
                child = Padding(
                  padding: const EdgeInsets.only(top: 16),
                  child: Text('Error: ${snapshot.error}'),
                );
              } else {
                child = const CircularProgressIndicator();
              }
              return Center(
                child: child,
              );
            }),
      );
    } else {
      return const Icon(Icons.file_present_rounded, size: 30);
    }
  }
}

String formatTime(DateTime? d) {
  if (d == null) return "-";
  return "${d.year.toString()}-${d.month.toString().padLeft(2, "0")}-${d.day.toString().padLeft(2, "0")} ${d.hour.toString().padLeft(2, "0")}:${d.minute.toString().padLeft(2, "0")}:${d.second.toString().padLeft(2, "0")}";
}

String? joinQueryParameters(Map<String, String> params) {
  return params.entries
      .map((MapEntry<String, String> e) => '${e.key}=${e.value}')
      .join('&');
}
