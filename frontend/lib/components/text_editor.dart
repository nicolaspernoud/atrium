import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:webdav_client/webdav_client.dart';

class TextEditor extends StatefulWidget {
  const TextEditor(
      {super.key,
      required this.client,
      required this.file,
      required this.readWrite});

  final Client client;
  final File file;
  final bool readWrite;

  @override
  State<TextEditor> createState() => _TextEditorState();
}

class _TextEditorState extends State<TextEditor> {
  late TextEditingController _editingController;

  bool _showTick = false;
  bool _saveError = false;

  void _showTickForDuration() {
    setState(() {
      _showTick = true;
    });

    Timer(const Duration(seconds: 3), () {
      setState(() {
        _showTick = false;
      });
    });
  }

  @override
  void initState() {
    super.initState();
    _editingController = TextEditingController(text: "");
    // Get file content and push it to the _controller
    getFileContent();
  }

  Future<void> getFileContent() async {
    var content = await widget.client.read(widget.file.path!);
    setState(() {
      _editingController.text = utf8.decode(content);
    });
  }

  @override
  void dispose() {
    _editingController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.file.name!),
      ),
      body: Padding(
        padding: const EdgeInsets.all(8.0),
        child: TextFormField(
          decoration: const InputDecoration(
            filled: true,
            isDense: true,
            border: InputBorder.none,
            contentPadding: EdgeInsets.symmetric(vertical: 4.0),
          ),
          controller: _editingController,
          cursorColor: Colors.blue,
          maxLines: null,
        ),
      ),
      bottomNavigationBar: widget.readWrite
          ? BottomAppBar(
              child: Row(children: [
              Stack(
                alignment: AlignmentDirectional.bottomEnd,
                children: [
                  AnimatedOpacity(
                    opacity: _showTick ? 1.0 : 0.0,
                    duration: const Duration(milliseconds: 500),
                    child: Icon(
                      _saveError ? Icons.priority_high : Icons.done,
                      color: _saveError ? Colors.red : Colors.green,
                    ),
                  ),
                  IconButton(
                      icon: const Icon(Icons.save),
                      onPressed: () async {
                        try {
                          await widget.client.write(
                              widget.file.path!,
                              Uint8List.fromList(
                                  utf8.encode(_editingController.text)));
                          _saveError = false;
                        } on Exception {
                          _saveError = true;
                          // Do nothing but do not display the tick
                        }
                        _showTickForDuration();
                      }),
                ],
              ),
            ]))
          : null,
    );
  }
}
