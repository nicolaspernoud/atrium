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
import 'package:flutter/scheduler.dart';
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
import 'adaptive_drag.dart';
import 'file_utils.dart';
import 'icons.dart';

enum SortBy { names, dates }

class Explorer extends StatefulWidget {
  late final String url;
  late final DavModel dav;
  final bool readWrite;
  final String? initialPath;
  // ignore: prefer_const_constructors_in_immutables
  Explorer({super.key, required this.dav, this.initialPath})
    : url = modelUrl(dav),
      readWrite = dav.writable;

  @override
  ExplorerState createState() => ExplorerState();
}

enum CopyMoveStatus { none, copy, move }

class ExplorerState extends State<Explorer> with TickerProviderStateMixin {
  late webdav.Client client;
  final user = 'dummy';
  final pwd = App().token;
  late String dirPath;
  var _copyMoveStatus = CopyMoveStatus.none;
  var _copyMovePaths = <String>[];
  final Set<String> _selected = {};
  var _selecting = false;
  late bool readWrite;
  late Future<List<File>> files;
  var sortBy = SortBy.names;
  var foundFile = "";

  final ItemScrollController itemScrollController = ItemScrollController();
  final ItemPositionsListener itemPositionsListener =
      ItemPositionsListener.create();
  final ScrollOffsetController _scrollOffsetController =
      ScrollOffsetController();
  final GlobalKey _listKey = GlobalKey();
  late final Ticker _autoScrollTicker;
  Offset? _dragLocation;

  @override
  void initState() {
    NotificationsPlugin(); // To ensure singleton is initialised
    super.initState();
    _autoScrollTicker = Ticker(_autoScrollTick);
    readWrite = widget.readWrite;
    dirPath = widget.initialPath ?? '/';
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
  void dispose() {
    _autoScrollTicker.dispose();
    super.dispose();
  }

  // While a file is dragged near the top or bottom edge of the list, the
  // list smoothly scrolls so that off-screen folders can be reached as a
  // drop target.
  void _autoScrollTick(Duration elapsed) {
    final location = _dragLocation;
    final renderBox = _listKey.currentContext?.findRenderObject() as RenderBox?;
    if (location == null || renderBox == null || !renderBox.hasSize) {
      return;
    }
    final offset = dragAutoScrollOffset(
      location,
      renderBox.localToGlobal(Offset.zero) & renderBox.size,
    );
    if (offset != null && offset != 0) {
      _scrollOffsetController.animateScroll(
        offset: offset,
        duration: const Duration(milliseconds: 16),
      );
    }
  }

  void _stopDragAutoScroll() {
    _dragLocation = null;
    _autoScrollTicker.stop();
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
            Expanded(
              child: Text(
                widget.dav.name,
                overflow: TextOverflow.ellipsis,
              ),
            ),
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
                    final error = snapshot.error;
                    if (error is DioException) {
                      if (error.response?.statusCode == 403) {
                        setCookie("ATRIUM_AUTH", App().token);
                        client.readDir(dirPath).then((value) {
                          files = Future.value(value);
                          setState(() {});
                        });
                      }
                    } else {
                      return Center(child: Text('Error: ${snapshot.error}'));
                    }
                  }
                  return _buildListView(context, snapshot.data ?? []);
              }
            },
      ),
      bottomNavigationBar: BottomAppBar(
        child: readWrite && _selected.isNotEmpty
            ? Row(
                children: [
                  IconButton(
                    icon: const Icon(Icons.refresh),
                    onPressed: () {
                      setState(() {
                        _getData();
                      });
                    },
                  ),
                  IconButton(
                    icon: const Icon(Icons.close),
                    tooltip: tr(context, "done"),
                    onPressed: () {
                      setState(() {
                        _selecting = false;
                        _selected.clear();
                      });
                    },
                  ),
                  Expanded(
                    child: Text(
                      "${_selected.length} ${tr(context, "selected")}",
                    ),
                  ),
                  IconButton(
                    icon: const Icon(Icons.copy),
                    tooltip: tr(context, "copy"),
                    onPressed: () => _startCopyMove(CopyMoveStatus.copy),
                  ),
                  IconButton(
                    icon: const Icon(Icons.content_cut),
                    tooltip: tr(context, "cut"),
                    onPressed: () => _startCopyMove(CopyMoveStatus.move),
                  ),
                  IconButton(
                    icon: const Icon(Icons.delete),
                    tooltip: tr(context, "delete"),
                    onPressed: _deleteSelected,
                  ),
                ],
              )
            : Row(
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
                                var result = await file_picker.FilePicker.pickFiles();
                                for (var file in result) {
                                  app.pushUpload(client, file, dirPath);
                                }
                                uploadStream = app.uploadAll();
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
                    Badge.count(
                      count: _copyMovePaths.length,
                      child: IconButton(
                        icon: const Icon(Icons.paste),
                        tooltip: tr(context, "paste"),
                        onPressed: _paste,
                      ),
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
        if (dirPath != "/" && dirPath != widget.initialPath)
          DragTarget<List<String>>(
            onWillAcceptWithDetails: (details) =>
                readWrite && _isDropOnFolderValid(p.dirname(dirPath), details.data),
            onAcceptWithDetails: (details) => _moveToParent(details.data),
            builder: (context, candidateData, acceptedData) {
              return ListTile(
                leading: const Icon(Icons.reply),
                title: const Text(".."),
                tileColor: candidateData.isNotEmpty
                    ? dropHighlightColor(context, strong: true)
                    : null,
                onTap: () {
                  dirPath = dirPath.substring(0, dirPath.length - 1);
                  dirPath = dirPath.substring(0, dirPath.lastIndexOf("/") + 1);
                  setState(() {
                    _getData();
                  });
                },
              );
            },
          ),
        Expanded(
          child: DragTarget<List<String>>(
            onWillAcceptWithDetails: (details) =>
                readWrite && _isDropOnFolderValid(dirPath, details.data),
            onAcceptWithDetails: (details) => _moveToFolder(dirPath, details.data),
            builder: (context, candidateData, acceptedData) {
              return ColoredBox(
                key: _listKey,
                color: candidateData.isNotEmpty
                    ? dropHighlightColor(context)
                    : Colors.transparent,
                child: ScrollablePositionedList.builder(
                  itemCount: list.length,
                  scrollOffsetController: _scrollOffsetController,
                  itemBuilder: (context, index) {
                    var file = list[index];
                    var type = fileType(file);
                    var isSelected = _selected.contains(file.path!);

                    var item = _buildTile(
                      context,
                      file,
                      type,
                      index,
                      list,
                      isSelected,
                      false,
                      cancelToken,
                    );
                    if (readWrite && file.isDir!) {
                      item = DragTarget<List<String>>(
                        onWillAcceptWithDetails: (details) =>
                            _isDropOnFolderValid(file.path!, details.data),
                        onAcceptWithDetails: (details) =>
                            _moveToFolder(file.path!, details.data),
                        builder: (context, candidateData, acceptedData) =>
                            _buildTile(
                                context,
                                file,
                                type,
                                index,
                                list,
                                isSelected,
                                candidateData.isNotEmpty,
                                cancelToken),
                      );
                    }
                    if (readWrite) {
                      final List<String> dragData = isSelected
                          ? _selected.toList()
                          : <String>[file.path!];
                      item = AdaptiveDraggable<List<String>>(
                        data: dragData,
                        onDragStarted: () {
                          _autoScrollTicker.start();
                        },
                        onDragUpdate: (details) {
                          _dragLocation = details.globalPosition;
                        },
                        onDragEnd: (_) => _stopDragAutoScroll(),
                        onDraggableCanceled: (_, _) => _stopDragAutoScroll(),
                        feedback: Material(
                          color: Theme.of(context).cardColor,
                          child: Container(
                            padding: const EdgeInsets.all(8),
                            child: Row(
                              mainAxisSize: MainAxisSize.min,
                              children: [
                                Icon(
                                  file.isDir!
                                      ? Icons.folder
                                      : Icons.file_present_rounded,
                                  size: 20,
                                ),
                                const SizedBox(width: 8),
                                Text(
                                  dragData.length == 1
                                      ? (file.name ?? '')
                                      : "${dragData.length} ${tr(context, "items")}",
                                ),
                              ],
                            ),
                          ),
                        ),
                        child: item,
                      );
                    }
                    return item;
                  },
                  itemScrollController: itemScrollController,
                  itemPositionsListener: itemPositionsListener,
                ),
              );
            },
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

  Widget _buildTile(
    BuildContext context,
    File file,
    FileType type,
    int index,
    List<File> list,
    bool isSelected,
    bool isDropTarget,
    CancelToken cancelToken,
  ) {
    Color? tileColor;
    if (isDropTarget) {
      tileColor = dropHighlightColor(context, strong: true);
    } else if (isSelected) {
      tileColor = Theme.of(context).colorScheme.secondaryContainer;
    } else if (file.path! == foundFile) {
      tileColor = Colors.grey[400];
    }

    final Widget popupMenu = PopupMenuButton(
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
                  builder: (context) => ShareDialog(
                    widget.url,
                    file,
                    client,
                    widget.dav.writable,
                  ),
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
                _copyMovePaths = [file.path!];
              });
            }),
            child: Row(
              children: [
                Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: Icon(
                    Icons.copy,
                    color:
                        _copyMovePaths.contains(file.path!) &&
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
                _copyMovePaths = [file.path!];
              });
            }),
            child: Row(
              children: [
                Padding(
                  padding: const EdgeInsets.all(8.0),
                  child: Icon(
                    Icons.cut,
                    color:
                        _copyMovePaths.contains(file.path!) &&
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
                    _selected.remove(file.path!);
                    _getData();
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
    );

    return ListTile(
      leading: _selecting
          ? Checkbox(
              value: isSelected,
              onChanged:
                  (bool? value) {
                    setState(() {
                      if (value ?? false) {
                        _selected.add(file.path!);
                      } else {
                        _selected.remove(file.path!);
                      }
                    });
                  },
            )
          : fileTypeWidget(client, file, type, cancelToken),
      tileColor: tileColor,
      title: Text(file.name ?? ''),
      subtitle: Text(
        formatTime(file.mTime) +
            ((file.size != null && file.size! > 0)
                ? " - ${filesize(file.size, 0)}"
                : ""),
      ),
      trailing: _selecting ? null : popupMenu,
      onTap: () async {
        if (_selecting) {
          setState(() {
            if (isSelected) {
              _selected.remove(file.path!);
            } else {
              _selected.add(file.path!);
            }
          });
          return;
        }
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
            var shareResponse = await ApiProvider().getShareToken(
              widget.url.split("://")[1].split(":")[0],
              file.path!,
              shareWith: "external_editor",
              shareForDays: 1,
              writable: readWrite,
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
                'share_token': shareResponse!.token,
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
              var shareResponse = await ApiProvider().getShareToken(
                widget.url.split("://")[1].split(":")[0],
                file.path!,
                shareWith: "media_player",
                shareForDays: 1,
              );
              uri = '$uri?token=${shareResponse!.token}';
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
  }

  void _startCopyMove(CopyMoveStatus status) {
    setState(() {
      _copyMoveStatus = status;
      _copyMovePaths = _selected.toList();
      _selected.clear();
      _selecting = false;
    });
  }

  Future<void> _paste() async {
    final CancelToken cancelToken = CancelToken();
    for (var src in _copyMovePaths) {
      // Never copy or move a folder into itself
      if (src.endsWith("/") && dirPath.startsWith(src)) {
        continue;
      }
      late String dest;
      // Case of directory
      if (src.endsWith("/")) {
        dest = "$dirPath${p.basename(src)}/";
      } else {
        dest = dirPath + p.basename(src);
      }
      if (dest == src) continue;
      try {
        if (_copyMoveStatus == CopyMoveStatus.copy) {
          await client.copy(src, dest, true, cancelToken);
        } else {
          await client.rename(src, dest, true, cancelToken);
        }
      } catch (e) {
        if (!mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(e.toString())),
        );
      }
    }
    setState(() {
      _copyMoveStatus = CopyMoveStatus.none;
      _copyMovePaths.clear();
      _getData();
    });
  }

  Future<void> _deleteSelected() async {
    final String label = _selected.length == 1
        ? _selected.first.split("/").last
        : "${_selected.length} ${tr(context, "items")}";
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => DeleteDialog(label),
    );
    if (confirmed ?? false) {
      for (final path in _selected.toList()) {
        await client.removeAll(path);
      }
      setState(() {
        _selected.clear();
        _selecting = false;
        _getData();
      });
    }
  }

  Future<void> _moveToParent(List<String> paths) {
    return _moveToFolder(p.dirname(dirPath), paths);
  }

  bool _isDropOnFolderValid(String destDir, List<String> paths) {
    if (paths.isEmpty) return false;
    final String dest = destDir.endsWith("/") ? destDir : "$destDir/";
    // A folder cannot be dropped on itself or in one of its subfolders
    return !paths.any((src) => src.endsWith("/") && dest.startsWith(src));
  }

  Future<void> _moveToFolder(String destDir, List<String> paths) async {
    final String dest = destDir.endsWith("/") ? destDir : "$destDir/";
    final CancelToken cancelToken = CancelToken();
    var moved = false;
    for (final src in paths) {
      // Never move a folder into itself or one of its subfolders
      if (src.endsWith("/") && dest.startsWith(src)) continue;
      final String name = p.basename(src);
      // WebDAV requires a trailing "/" on the destination of a collection
      final String target = src.endsWith("/") ? "$dest$name/" : "$dest$name";
      if (target == src) continue;
      try {
        await client.rename(src, target, true, cancelToken);
        moved = true;
      } catch (e) {
        if (!mounted) return;
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text(e.toString())),
        );
      }
    }
    if (moved) {
      setState(() {
        _selected.clear();
        _getData();
      });
    }
  }

  List<Widget> get explorerActions {
    return <Widget>[
      if (readWrite)
        IconButton(
          tooltip: tr(context, "select"),
          icon: Icon(_selecting ? Icons.deselect : Icons.checklist),
          onPressed: () {
            setState(() {
              _selecting = !_selecting;
              if (!_selecting) {
                _selected.clear();
              }
            });
          },
        ),
      if (_selecting)
        IconButton(
          tooltip: tr(context, "select_all"),
          icon: const Icon(Icons.select_all),
          onPressed: () {
            files.then((list) {
              setState(() {
                _selected.addAll(list.map((f) => f.path!));
              });
            });
          },
        ),
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
