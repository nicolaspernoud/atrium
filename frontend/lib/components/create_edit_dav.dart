import 'package:atrium/components/create_edit_app.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/dav.dart';
import 'package:atrium/utils.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../i18n.dart';
import 'icons.dart';

class CreateEditDav extends StatefulWidget {
  final DavModel dav;
  final bool isNew;
  const CreateEditDav({super.key, required this.dav, required this.isNew});

  @override
  CreateEditDavState createState() => CreateEditDavState();
}

class CreateEditDavState extends State<CreateEditDav> {
  final _formKey = GlobalKey<FormState>();
  final _passController = TextEditingController();
  bool submitting = false;

  @override
  void initState() {
    super.initState();
    _passController.text = widget.dav.passphrase ?? "";
    _passController.addListener(() {
      widget.dav.passphrase = _passController.text;
    });
  }

  @override
  void dispose() {
    _passController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    // Build a Form widget using the _formKey created above.
    var intOnly =
        FilteringTextInputFormatter.allow(RegExp(r'^(?:0|[1-9][0-9]*)$'));
    // ignore: prefer_function_declarations_over_variables
    var rejectEmpty = (value) {
      if (value == null || value.isEmpty) {
        return tr(context, "please_enter_some_text");
      }
      return null;
    };

    return Scaffold(
        appBar: AppBar(
          title: !widget.isNew
              ? Text(tr(context, "edit_dav"))
              : Text(tr(context, "new_dav")),
          actions: !widget.isNew
              ? [
                  IconButton(
                      icon: const Icon(Icons.delete_forever),
                      onPressed: () async {
                        await ApiProvider().deleteDav(widget.dav.id);
                        if (!context.mounted) return;
                        Navigator.pop(context);
                        ScaffoldMessenger.of(context).showSnackBar(SnackBar(
                            content: Text(MyLocalizations.of(context)!
                                .tr("dav_deleted"))));
                      })
                ]
              : null,
        ),
        body: ListView(
          children: [
            Padding(
                padding: const EdgeInsets.all(16.0),
                child: Form(
                  key: _formKey,
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      TextFormField(
                        initialValue: widget.dav.id.toString(),
                        decoration:
                            InputDecoration(labelText: tr(context, "id")),
                        keyboardType: TextInputType.number,
                        inputFormatters: [intOnly],
                        validator: rejectEmpty,
                        onChanged: (value) {
                          widget.dav.id = int.parse(value);
                        },
                      ),
                      TextFormField(
                        initialValue: widget.dav.name,
                        decoration:
                            InputDecoration(labelText: tr(context, "name")),
                        validator: rejectEmpty,
                        onChanged: (value) {
                          widget.dav.name = value;
                        },
                      ),
                      Center(
                        child: Padding(
                          padding: const EdgeInsets.all(16.0),
                          child: IconButton(
                              iconSize: 40.0,
                              onPressed: () async {
                                await pickIcon(context, widget.dav);
                                setState(() {});
                              },
                              icon: Icon(roundedIcons[widget.dav.icon],
                                  color: widget.dav.color)),
                        ),
                      ),
                      Row(
                        children: [
                          Checkbox(
                            value: widget.dav.writable,
                            onChanged: (bool? value) async {
                              if (value != null) {
                                widget.dav.writable = value;
                                setState((() {}));
                              }
                            },
                          ),
                          Padding(
                            padding: const EdgeInsets.all(16),
                            child: Text(tr(context, "is_writable")),
                          ),
                        ],
                      ),
                      TextFormField(
                        initialValue: widget.dav.host,
                        decoration:
                            InputDecoration(labelText: tr(context, "host")),
                        validator: rejectEmpty,
                        onChanged: (value) {
                          widget.dav.host = value;
                        },
                      ),
                      TextFormField(
                        initialValue: widget.dav.directory,
                        decoration:
                            InputDecoration(labelText: tr(context, "target")),
                        validator: rejectEmpty,
                        onChanged: (value) {
                          widget.dav.directory = value;
                        },
                      ),
                      Row(
                        children: [
                          Checkbox(
                            value: widget.dav.secured,
                            onChanged: (bool? value) async {
                              if (value != null) {
                                widget.dav.secured = value;
                                setState((() {}));
                              }
                            },
                          ),
                          Padding(
                            padding: const EdgeInsets.all(16),
                            child: Text(tr(context, "secured")),
                          ),
                        ],
                      ),
                      Row(
                        children: [
                          Checkbox(
                            value: widget.dav.allowSymlinks,
                            onChanged: (bool? value) async {
                              if (value != null) {
                                widget.dav.allowSymlinks = value;
                                setState((() {}));
                              }
                            },
                          ),
                          Padding(
                            padding: const EdgeInsets.all(16),
                            child: Text(tr(context, "allow_symlinks")),
                          ),
                        ],
                      ),
                      TextFormField(
                        controller: _passController,
                        decoration: InputDecoration(
                            labelText: tr(context, "passphrase"),
                            suffixIcon: IconButton(
                                icon: const Icon(Icons.casino),
                                onPressed: () {
                                  _passController.text =
                                      generateRandomString(48);
                                })),
                      ),
                      TextFormField(
                        initialValue: widget.dav.roles.join(","),
                        decoration:
                            InputDecoration(labelText: tr(context, "roles")),
                        onChanged: (value) {
                          widget.dav.roles = value.split(",");
                        },
                      ),
                      Padding(
                          padding: const EdgeInsets.symmetric(vertical: 16.0),
                          child: AnimatedSwitcher(
                            duration: const Duration(milliseconds: 1000),
                            child: !submitting
                                ? ElevatedButton(
                                    onPressed: () async {
                                      // Validate returns true if the form is valid, or false otherwise.
                                      if (_formKey.currentState!.validate()) {
                                        var msg = tr(context, "dav_created");
                                        try {
                                          setState(() {
                                            submitting = true;
                                          });
                                          await ApiProvider()
                                              .createDav(widget.dav);
                                          // Do nothing on TypeError as Create respond with a null id
                                        } catch (e) {
                                          msg = e.toString();
                                        }
                                        if (!context.mounted) return;
                                        ScaffoldMessenger.of(context)
                                            .showSnackBar(
                                          SnackBar(content: Text(msg)),
                                        );
                                        Navigator.pop(context);
                                      }
                                    },
                                    child: Padding(
                                      padding: const EdgeInsets.all(16.0),
                                      child: Text(tr(context, "submit")),
                                    ),
                                  )
                                : const Center(
                                    child: CircularProgressIndicator()),
                          )),
                    ],
                  ),
                )),
          ],
        ));
  }
}
