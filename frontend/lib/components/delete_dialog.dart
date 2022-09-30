import 'package:atrium/i18n.dart';
import 'package:flutter/material.dart';

class DeleteDialog extends StatefulWidget {
  final String name;
  const DeleteDialog(this.name, {Key? key}) : super(key: key);

  @override
  State<DeleteDialog> createState() => _DeleteDialogState();
}

class _DeleteDialogState extends State<DeleteDialog> {
  late bool confirmed;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Column(
        children: [
          Text("${tr(context, "confirm_deletion_of")} ${widget.name} ?"),
          const SizedBox(height: 15),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
            children: [
              ElevatedButton(
                style: ElevatedButton.styleFrom(
                  primary: Colors.red, // background
                ),
                onPressed: () => Navigator.pop(context, true),
                child: Padding(
                  padding: const EdgeInsets.all(12.0),
                  child: Text(tr(context, "ok")),
                ),
              ),
              ElevatedButton(
                style: ElevatedButton.styleFrom(
                  primary: Colors.green, // background
                ),
                onPressed: () => Navigator.pop(context, false),
                child: Padding(
                  padding: const EdgeInsets.all(12.0),
                  child: Text(tr(context, "cancel")),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}
