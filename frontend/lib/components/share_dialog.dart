import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:webdav_client/webdav_client.dart';

class ShareDialog extends StatefulWidget {
  final String url;
  final File file;
  const ShareDialog(this.url, this.file, {Key? key}) : super(key: key);

  @override
  State<ShareDialog> createState() => _ShareDialogState();
}

class _ShareDialogState extends State<ShareDialog> {
  String _shareWith = "";
  double _shareForDays = 10;

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
        const SizedBox(height: 15),
        ElevatedButton(
          onPressed: (_shareForDays == 0)
              ? null
              : () async {
                  var shareToken = await ApiProvider().getShareToken(
                      widget.url.split("://")[1].split(":")[0],
                      widget.file.path!,
                      shareWith: _shareWith,
                      shareForDays: _shareForDays.round());
                  var shareUrl =
                      '${widget.url}${widget.file.path}?token=$shareToken';
                  Clipboard.setData(ClipboardData(text: shareUrl));
                  if (!mounted) return;
                  ScaffoldMessenger.of(context).showSnackBar(
                      SnackBar(content: Text(tr(context, "share_url_copied"))));
                  if (!mounted) return;
                  Navigator.pop(context);
                },
          child: Padding(
            padding: const EdgeInsets.all(12.0),
            child: Text(tr(context, "ok")),
          ),
        ),
      ]),
    );
  }
}
