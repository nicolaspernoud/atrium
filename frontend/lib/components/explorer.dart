import 'package:atrium/components/delete_dialog.dart';
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
import 'package:atrium/models/pathitem.dart';
import 'package:atrium/utils.dart';
import 'package:dio/dio.dart';
import 'package:file_picker/file_picker.dart' as file_picker;
import 'package:filesize/filesize.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:scrollable_positioned_list/scrollable_positioned_list.dart';
import 'package:webdav_client/webdav_client.dart' as webdav;
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';
import 'package:webdav_client/webdav_client.dart';
import 'package:atrium/components/webview.dart'
    if (dart.library.html) 'package:atrium/components/iframe_webview.dart';
import 'package:path/path.dart' as p;
import 'icons.dart';

enum SortBy { names, dates }

class Explorer extends StatefulWidget {
  late final String url;
  late final DavModel dav;
  final bool readWrite;
  // ignore: prefer_const_constructors_in_immutables
  Explorer({super.key, required this.dav})
    : url = modelUrl(dav),
      readWrite = dav.writable;

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
  var sortBy = SortBy.names;
  var foundFile = "";

  final ItemScrollController itemScrollController = ItemScrollController();
  final ItemPositionsListener itemPositionsListener =
      ItemPositionsListener.create();

  @override
  void initState() {
    NotificationsPlugin(); // To ensure singleton is initialised
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
            Icon(roundedIcons[widget.dav.icon], size: 30),
            const SizedBox(width: 15),
            Text(widget.dav.name),
          ],
        ),
        actions: explorerActions,
      ),
      body: FutureBuilder(
        future: files,
        builder:
            (BuildContext context, AsyncSnapshot<List<webdav.File>> snapshot) {
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
            },
      ),
      bottomNavigationBar: BottomAppBar(
        child: Row(
          children: [
            IconButton(
              icon: const Icon(Icons.refresh),
              onPressed: () {
                setState(() {
                  _getData();
                });
              },
            ),
            if (readWrite)
              IconButton(
                icon: const Icon(Icons.create_new_folder),
                onPressed: () async {
                  CancelToken c = CancelToken();
                  await client.mkdir("$dirPath/newfolder", c);
                  setState(() {
                    _getData();
                  });
                },
              ),
            if (readWrite)
              IconButton(
                icon: const Icon(Icons.add),
                onPressed: () async {
                  CancelToken c = CancelToken();
                  await client.write(
                    "$dirPath/newfile.txt",
                    Uint8List(0),
                    onProgress: (c, t) {
                      debugPrint((c / t).toString());
                    },
                    cancelToken: c,
                  );
                  setState(() {
                    _getData();
                  });
                },
              ),
            if (readWrite)
              Consumer<App>(
                builder: (context, app, child) {
                  var uploadStream = app.uploadAll();
                  uploadStream.listen((up) {
                    if (up != null && dirPath == up.destPath) {
                      setState(() {
                        _getData();
                      });
                    }
                  });
                  return Row(
                    children: [
                      IconButton(
                        icon: const Icon(Icons.upload),
                        onPressed: () async {
                          file_picker.FilePickerResult? result =
                              await file_picker.FilePicker.platform.pickFiles(
                                allowMultiple: true,
                                withReadStream: true,
                              );
                          if (result != null) {
                            for (var file in result.files) {
                              app.pushUpload(client, file, dirPath);
                            }
                            uploadStream = app.uploadAll();
                          }
                        },
                      ),
                      if (app.hasUploads)
                        IconButton(
                          icon: const Icon(Icons.playlist_add_check),
                          onPressed: () {
                            showModalBottomSheet<void>(
                              context: context,
                              builder: (BuildContext context) {
                                return const UploadsList();
                              },
                            );
                          },
                        ),
                    ],
                  );
                },
              ),
            if (_copyMoveStatus != CopyMoveStatus.none)
              IconButton(
                icon: const Icon(Icons.paste),
                onPressed: () async {
                  CancelToken c = CancelToken();
                  String dest;
                  // Case of directory
                  if (_copyMovePath.endsWith("/")) {
                    dest =
                        dirPath +
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
                },
              ),
          ],
        ),
      ),
    );
  }

  void _getData() {
    files = client.readDir(dirPath);
  }

  Widget _buildListView(BuildContext context, List<webdav.File> list) {
    if (sortBy == SortBy.names) {
      list.sort(foldersFirstThenAlphabetically);
    } else {
      list.sort((a, b) => b.mTime!.compareTo(a.mTime!));
    }

    for (var i = 0; i < list.length; i++) {
      // If we found a file before building this view, we scroll to that file
      if (list[i].path! == foundFile) {
        WidgetsBinding.instance.addPostFrameCallback((_) {
          itemScrollController.jumpTo(index: i);
        });
        break;
      }
    }

    CancelToken cancelToken = CancelToken();

    return Column(
      children: [
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
        Expanded(
          child: ScrollablePositionedList.builder(
            itemCount: list.length,
            itemBuilder: (context, index) {
              var file = list[index];
              var type = fileType(file);

              return ListTile(
                leading: widgetFromFileType(file, type, cancelToken),
                tileColor: file.path! == foundFile ? Colors.grey[400] : null,
                title: Text(file.name ?? ''),
                subtitle: Text(
                  formatTime(file.mTime) +
                      ((file.size != null && file.size! > 0)
                          ? " - ${filesize(file.size, 0)}"
                          : ""),
                ),
                trailing: PopupMenuButton(
                  itemBuilder: (BuildContext context) => <PopupMenuEntry>[
                    PopupMenuItem(
                      onTap: () => download(widget.url, client, file, context),
                      child: Row(
                        children: [
                          const Padding(
                            padding: EdgeInsets.all(8.0),
                            child: Icon(Icons.download),
                          ),
                          Text(tr(context, "download")),
                        ],
                      ),
                    ),
                    PopupMenuItem(
                      onTap: () {
                        WidgetsBinding.instance.addPostFrameCallback((_) async {
                          if (!widget.dav.secured) {
                            Clipboard.setData(
                              ClipboardData(
                                text: '${widget.url}${escapePath(file.path!)}',
                              ),
                            );
                            ScaffoldMessenger.of(context).showSnackBar(
                              SnackBar(
                                content: Text(tr(context, "share_url_copied")),
                              ),
                            );
                          } else {
                            await showDialog(
                              context: context,
                              builder: (context) =>
                                  ShareDialog(widget.url, file, client),
                            );
                          }
                        });
                      },
                      child: Row(
                        children: [
                          const Padding(
                            padding: EdgeInsets.all(8.0),
                            child: Icon(Icons.share),
                          ),
                          Text(tr(context, "share")),
                        ],
                      ),
                    ),
                    if (readWrite) ...[
                      PopupMenuItem(
                        onTap: () {
                          WidgetsBinding.instance.addPostFrameCallback((
                            _,
                          ) async {
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
                            Text(tr(context, "rename")),
                          ],
                        ),
                      ),
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
                              child: Icon(
                                Icons.copy,
                                color:
                                    _copyMovePath == file.path! &&
                                        _copyMoveStatus == CopyMoveStatus.copy
                                    ? Colors.blueAccent
                                    : null,
                              ),
                            ),
                            Text(tr(context, "copy")),
                          ],
                        ),
                      ),
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
                              child: Icon(
                                Icons.cut,
                                color:
                                    _copyMovePath == file.path! &&
                                        _copyMoveStatus == CopyMoveStatus.move
                                    ? Colors.blueAccent
                                    : null,
                              ),
                            ),
                            Text(tr(context, "cut")),
                          ],
                        ),
                      ),
                      PopupMenuItem(
                        onTap: () async {
                          WidgetsBinding.instance.addPostFrameCallback((
                            _,
                          ) async {
                            var confirmed = await showDialog<bool>(
                              context: context,
                              builder: (context) => DeleteDialog(file.name!),
                            );
                            if (confirmed!) {
                              await client.removeAll(file.path!);
                              setState(() {
                                list.removeAt(index);
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
                            Text(tr(context, "delete")),
                          ],
                        ),
                      ),
                    ],
                  ],
                ),
                onTap: () async {
                  if (file.isDir!) {
                    dirPath = file.path!;
                    setState(() {
                      _getData();
                    });
                  } else {
                    if (type == FileType.text) {
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => TextEditor(
                            client: client,
                            file: file,
                            readWrite: readWrite,
                          ),
                        ),
                      );
                    } else if (type == FileType.image) {
                      cancelToken.cancel();
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => ImageViewer(
                            client: client,
                            url: widget.url,
                            files: list,
                            index: index,
                          ),
                        ),
                      );
                    } else if (type == FileType.pdf) {
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => PdfViewer(
                            client: client,
                            url: widget.url,
                            file: file,
                            color: widget.dav.color,
                          ),
                        ),
                      );
                    } else if (type == FileType.document) {
                      // Get a share token for this document
                      var shareToken = await ApiProvider().getShareToken(
                        widget.url.split("://")[1].split(":")[0],
                        file.path!,
                        shareWith: "external_editor",
                        shareForDays: 1,
                      );
                      final Uri launchUri = Uri(
                        scheme: App().prefs.hostnameScheme,
                        host: App().prefs.hostnameHost,
                        port: App().prefs.hostnamePort,
                        path: 'onlyoffice',
                        query: joinQueryParameters(<String, String>{
                          'file': '${widget.url}${file.path}',
                          'mtime': file.mTime!.toIso8601String(),
                          'user': App().prefs.username,
                          'share_token': shareToken!,
                        }),
                      );
                      if (!context.mounted) return;
                      Navigator.of(context).push(
                        MaterialPageRoute<void>(
                          builder: (context) {
                            return Scaffold(
                              appBar: AppBar(toolbarHeight: 0.0),
                              body: AppWebView(
                                initialUrl: launchUri.toString(),
                              ),
                            );
                          },
                        ),
                      );
                    } else if (type == FileType.media) {
                      String uri = '${widget.url}${escapePath(file.path!)}';
                      if (kIsWeb) {
                        var shareToken = await ApiProvider().getShareToken(
                          widget.url.split("://")[1].split(":")[0],
                          file.path!,
                          shareWith: "media_player",
                          shareForDays: 1,
                        );
                        uri = '$uri?token=$shareToken';
                      }
                      if (!context.mounted) return;
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) =>
                              MediaPlayer(uri: uri, file: file),
                        ),
                      );
                    }
                  }
                },
              );
            },
            itemScrollController: itemScrollController,
            itemPositionsListener: itemPositionsListener,
          ),
        ),
        Padding(
          padding: const EdgeInsets.all(8.0),
          child: Text(
            "${list.length} ${tr(context, "item")}${list.length != 1 ? "s" : ""}",
          ),
        ),
      ],
    );
  }

  Widget widgetFromFileType(File file, FileType type, CancelToken cancelToken) {
    if (file.isDir != null && file.isDir!) {
      return const Icon(Icons.folder, size: 30);
    }
    switch (type) {
      case FileType.document:
        return const Icon(Icons.article, size: 30);
      case FileType.image:
        return SizedBox(
          width: 30,
          height: 30,
          child: FutureBuilder<ImageProvider>(
            future: client
                .read(file.path!, cancelToken: cancelToken)
                .then((value) => MemoryImage(Uint8List.fromList(value))),
            builder:
                (BuildContext context, AsyncSnapshot<ImageProvider> snapshot) {
                  if (snapshot.hasData) {
                    return Center(child: Image(image: snapshot.data!));
                  } else {
                    return const Icon(Icons.image, size: 30);
                  }
                },
          ),
        );
      case FileType.media:
        return const Icon(Icons.play_circle, size: 30);
      case FileType.pdf:
        return const Icon(Icons.picture_as_pdf, size: 30);
      case FileType.text:
        return const Icon(Icons.description, size: 30);
      default:
        return const Icon(Icons.file_present_rounded, size: 30);
    }
  }

  List<Widget> get explorerActions {
    return <Widget>[
      IconButton(
        onPressed: () async {
          String result = await showSearch(
            context: context,
            delegate: ExplorerSearchDelegate(widget.dav, dirPath),
          );
          dirPath = p.dirname(result);
          foundFile = result;
          setState(() {
            _getData();
          });
        },
        icon: const Icon(Icons.search),
      ),
      PopupMenuButton<SortBy>(
        tooltip: tr(context, "sort_by"),
        icon: const Icon(Icons.sort),
        onSelected: (SortBy item) {
          setState(() {
            sortBy = item;
          });
        },
        itemBuilder: (BuildContext context) => <PopupMenuEntry<SortBy>>[
          PopupMenuItem<SortBy>(
            value: SortBy.names,
            child: Row(
              children: [
                const Padding(
                  padding: EdgeInsets.all(8.0),
                  child: Icon(
                    color: Color.fromARGB(255, 140, 140, 140),
                    Icons.sort_by_alpha,
                  ),
                ),
                Text(tr(context, "names")),
              ],
            ),
          ),
          PopupMenuItem<SortBy>(
            value: SortBy.dates,
            child: Row(
              children: [
                const Padding(
                  padding: EdgeInsets.all(8.0),
                  child: Icon(
                    color: Color.fromARGB(255, 140, 140, 140),
                    Icons.sort,
                  ),
                ),
                Text(tr(context, "dates")),
              ],
            ),
          ),
        ],
      ),
    ];
  }
}

class ExplorerSearchDelegate extends SearchDelegate {
  DavModel dav;
  String dirPath;
  ExplorerSearchDelegate(this.dav, this.dirPath);

  @override
  List<Widget>? buildActions(BuildContext context) => [
    IconButton(
      onPressed: () {
        query = '';
      },
      icon: const Icon(Icons.clear),
    ),
  ];

  @override
  Widget? buildLeading(BuildContext context) => IconButton(
    onPressed: () => close(context, null),
    icon: const Icon(Icons.arrow_back),
  );

  @override
  Widget buildSuggestions(BuildContext context) {
    if (query.length < 3) {
      return Center(child: Text(tr(context, "at_least_3_chars")));
    }
    return FutureBuilder<List<PathItem>>(
      future: ApiProvider()
          .searchDav(dav, dirPath, query)
          .then(
            (value) => value
                .where(
                  (element) =>
                      element.name.toLowerCase().contains(query.toLowerCase()),
                )
                .toList(),
          ),
      builder: (BuildContext context, AsyncSnapshot<List<PathItem>> snapshot) {
        Widget child;
        if (snapshot.hasData) {
          child = ListView.builder(
            itemCount: snapshot.data!.length,
            itemBuilder: (context, index) {
              final suggestion = snapshot.data![index];
              return ListTile(
                leading: Icon(
                  suggestion.pathType == PathType.dir
                      ? Icons.folder
                      : Icons.file_present_rounded,
                  size: 30,
                ),
                title: Text(suggestion.name),
                onTap: () {
                  close(
                    context,
                    "$dirPath${suggestion.name}${suggestion.pathType == PathType.dir ? "/" : ""}",
                  );
                },
              );
            },
          );
        } else if (snapshot.hasError) {
          child = Padding(
            padding: const EdgeInsets.only(top: 16),
            child: Text('Error: ${snapshot.error}'),
          );
        } else {
          child = const SizedBox(
            width: 60,
            height: 60,
            child: CircularProgressIndicator(),
          );
        }
        return Center(child: child);
      },
    );
  }

  @override
  Widget buildResults(BuildContext context) {
    return buildSuggestions(context);
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

FileType fileType(File? file) {
  if (file == null || file.name == null) return FileType.other;
  var ext = file.name!.split(".").last.toLowerCase();
  return fileTypeFromExt(ext);
}

// Sort : folders first and then alphabetically
int foldersFirstThenAlphabetically(webdav.File a, webdav.File b) {
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
}
