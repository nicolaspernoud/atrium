import 'package:atrium/i18n.dart';
import 'package:flutter/material.dart';

class RenameDialog extends StatefulWidget {
  final String name;
  const RenameDialog(this.name, {Key? key}) : super(key: key);

  @override
  State<RenameDialog> createState() => _RenameDialogState();
}

class _RenameDialogState extends State<RenameDialog> {
  late String newName;
  final _controller = TextEditingController();

  @override
  void initState() {
    super.initState();
    newName = widget.name;
    _controller.text = newName;
    _controller.selection = TextSelection(
        baseOffset: 0,
        extentOffset:
            newName.contains(".") ? newName.lastIndexOf(".") : newName.length);
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Column(
        children: [
          TextFormField(
            controller: _controller,
            autofocus: true,
            decoration: InputDecoration(labelText: tr(context, "new_name")),
            onChanged: (value) {
              newName = value;
            },
          ),
          const SizedBox(height: 15),
          ElevatedButton(
            onPressed: () => Navigator.pop(context, newName),
            child: Padding(
              padding: const EdgeInsets.all(12.0),
              child: Text(tr(context, "ok")),
            ),
          ),
        ],
      ),
    );
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }
}
