import 'package:atrium/components/async_button.dart';
import 'package:atrium/components/explorer.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:rich_clipboard/rich_clipboard.dart';
import 'package:webdav_client/webdav_client.dart';

class ShareDialog extends StatefulWidget {
  final String url;
  final File file;
  final Client client;
  const ShareDialog(this.url, this.file, this.client, {super.key});

  @override
  State<ShareDialog> createState() => _ShareDialogState();
}

class _ShareDialogState extends State<ShareDialog> {
  String _shareWith = "";
  double _shareForDays = 10;
  late bool _isDir;
  bool _doNotZipFolder = false;

  @override
  void initState() {
    super.initState();
    _isDir = widget.file.isDir != null && widget.file.isDir!;
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Column(children: [
        TextFormField(
          initialValue: _shareWith,
          autofocus: true,
          decoration: InputDecoration(labelText: tr(context, "share_with")),
          onChanged: (value) {
            _shareWith = value;
          },
        ),
        Row(
          children: [
            Text(
              '${tr(context, "days")}: $_shareForDays',
              style: const TextStyle(fontSize: 12),
            ),
            Slider(
              value: _shareForDays,
              max: 100,
              min: 0,
              divisions: 10,
              label: _shareForDays.round().toString(),
              onChanged: (double value) {
                setState(() {
                  _shareForDays = value;
                });
              },
            ),
          ],
        ),
        if (_isDir)
          Row(
            children: [
              Checkbox(
                value: _doNotZipFolder,
                onChanged: (bool? newValue) {
                  setState(() {
                    _doNotZipFolder = newValue!;
                  });
                },
              ),
              Flexible(
                child: Text(tr(context, "do_not_zip_folder"),
                    style: const TextStyle(fontSize: 12.0)),
              )
            ],
          ),
        const SizedBox(height: 15),
        AsyncButton(
          buttonText: "OK",
          onPressed: (_shareForDays == 0)
              ? null
              : () async {
                  try {
                    if (_doNotZipFolder) {
                      var html = await downloadFolderAsHTMLList(widget.file);
                      await RichClipboard.setData(RichClipboardData(
                        text: html,
                        html: html,
                      ));
                    } else {
                      Clipboard.setData(ClipboardData(
                          text: await downloadSingleFile(widget.file.path!)));
                    }
                    if (!context.mounted) return; 
                    ScaffoldMessenger.of(context).showSnackBar(SnackBar(
                        content: Text(tr(context, "share_url_copied"))));
                  } on Exception {
                    if (!mounted) return;
                    ScaffoldMessenger.of(context).showSnackBar(SnackBar(
                        content: Text(tr(context, "failed_share_token"))));
                  }
                  if (!mounted) return;
                  Navigator.pop(context);
                },
        ),
      ]),
    );
  }

  Future<String> downloadSingleFile(String path) async {
    var shareToken = await ApiProvider().getShareToken(
        widget.url.split("://")[1].split(":")[0], path,
        shareWith: _shareWith, shareForDays: _shareForDays.round());
    var shareUrl = '${widget.url}${escapePath(path)}?token=$shareToken';
    return shareUrl;
  }

  Future<String> downloadFolderAsHTMLList(File file,
      {int recursionLevel = 0}) async {
    var files = await widget.client.readDir(file.path!);
    files.sort(foldersFirstThenAlphabetically);

    Future<String> fileList(File file) async {
      String content;
      if (recursionLevel < 2 && file.isDir != null && file.isDir!) {
        content = await downloadFolderAsHTMLList(file,
            recursionLevel: recursionLevel + 1);
      } else {
        var url = await downloadSingleFile(file.path!);
        content =
            '${file.isDir ?? false ? "📁 " : ""}<a href="$url">${file.name}</a>';
      }
      return '<li>$content</li>';
    }

    List<Future<String>> futures = List.generate(files.length, (index) {
      return fileList(files[index]);
    });

    List<String> htmlFiles = await Future.wait(futures);

    var folderAsHTML = '📁 ${file.name}<ul>${htmlFiles.join("\n")}</ul>';
    // Create an html list with those urls
    if (recursionLevel == 0) {
      folderAsHTML = '<html><body>$folderAsHTML</body></html>';
    }
    return folderAsHTML;
  }
}
