import 'package:atrium/i18n.dart';
import 'package:flutter/material.dart';

Future<String?> showRenameDialog(BuildContext context, String name) {
  return showDialog<String>(
    context: context,
    builder: (BuildContext context) => RenameDialog(name),
  );
}

class RenameDialog extends StatefulWidget {
  final String name;
  const RenameDialog(this.name, {super.key});

  @override
  State<RenameDialog> createState() => _RenameDialogState();
}

class _RenameDialogState extends State<RenameDialog> {
  late String newName;
  final _controller = TextEditingController();
  final _focusNode = FocusNode();
  bool _previouslyFocused = false;

  @override
  void initState() {
    super.initState();
    newName = widget.name;
    _controller.text = newName;
    _focusNode.addListener(_onFocusChange);
  }

  void _onFocusChange() {
    if (_focusNode.hasFocus && !_previouslyFocused) {
      _previouslyFocused = true;
      _selectNameWithoutExtension();
    } else if (!_focusNode.hasFocus) {
      _previouslyFocused = false;
    }
  }

  void _selectNameWithoutExtension() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      final text = _controller.text;
      final dotIndex = text.contains(".") ? text.lastIndexOf(".") : text.length;
      _controller.selection = TextSelection(
        baseOffset: 0,
        extentOffset: dotIndex,
      );
    });
  }

  void _submit() {
    Navigator.pop(context, newName);
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      content: SizedBox(
        width: 350,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              TextFormField(
                controller: _controller,
                focusNode: _focusNode,
                decoration: InputDecoration(labelText: tr(context, "new_name")),
                onChanged: (value) {
                  newName = value;
                },
                onTap: () {
                  if (!_previouslyFocused) {
                    _previouslyFocused = true;
                    _selectNameWithoutExtension();
                  }
                },
                onFieldSubmitted: (value) {
                  _submit();
                },
              ),
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: Padding(
            padding: const EdgeInsets.all(12.0),
            child: Text(tr(context, "cancel")),
          ),
        ),
        ElevatedButton(
          onPressed: _submit,
          child: Padding(
            padding: const EdgeInsets.all(12.0),
            child: Text(tr(context, "ok")),
          ),
        ),
      ],
    );
  }

  @override
  void dispose() {
    _focusNode.removeListener(_onFocusChange);
    _focusNode.dispose();
    _controller.dispose();
    super.dispose();
  }
}
