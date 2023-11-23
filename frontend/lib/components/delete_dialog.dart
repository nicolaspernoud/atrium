import 'package:atrium/i18n.dart';
import 'package:flutter/material.dart';

class DeleteDialog extends StatefulWidget {
  final String name;
  const DeleteDialog(this.name, {super.key});

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
                  backgroundColor: Colors.red, // background
                ),
                onPressed: () => Navigator.pop(context, true),
                child: Padding(
                  padding: const EdgeInsets.all(12.0),
                  child: Text(tr(context, "delete")),
                ),
              ),
              ElevatedButton(
                style: ElevatedButton.styleFrom(
                  backgroundColor: Colors.green, // background
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

class DeletingSpinner extends StatelessWidget {
  const DeletingSpinner({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    return const SizedBox(
      width: 30,
      height: 30,
      child: CircularProgressIndicator(
          valueColor: AlwaysStoppedAnimation<Color>(Colors.grey)),
    );
  }
}
