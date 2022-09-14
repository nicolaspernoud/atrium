import 'package:atrium/components/create_edit_app.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/dav.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../i18n.dart';

class CreateEditDav extends StatefulWidget {
  final DavModel dav;
  final bool isNew;
  const CreateEditDav({Key? key, required this.dav, required this.isNew})
      : super(key: key);

  @override
  CreateEditDavState createState() => CreateEditDavState();
}

class CreateEditDavState extends State<CreateEditDav> {
  final _formKey = GlobalKey<FormState>();

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
                        if (!mounted) return;
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
                              icon: Icon(
                                  IconData(widget.dav.icon,
                                      fontFamily: 'MaterialIcons'),
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
                        initialValue: widget.dav.passphrase,
                        decoration: InputDecoration(
                            labelText: tr(context, "passphrase")),
                        onChanged: (value) {
                          widget.dav.passphrase = value;
                        },
                      ),
                      TextFormField(
                        initialValue: widget.dav.roles.join(","),
                        decoration:
                            InputDecoration(labelText: tr(context, "roles")),
                        validator: rejectEmpty,
                        onChanged: (value) {
                          widget.dav.roles = value.split(",");
                        },
                      ),
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 16.0),
                        child: ElevatedButton(
                          onPressed: () async {
                            // Validate returns true if the form is valid, or false otherwise.
                            if (_formKey.currentState!.validate()) {
                              var msg = tr(context, "dav_created");
                              try {
                                await ApiProvider().createDav(widget.dav);
                                // Do nothing on TypeError as Create respond with a null id
                              } catch (e) {
                                msg = e.toString();
                              }
                              if (!mounted) return;
                              ScaffoldMessenger.of(context).showSnackBar(
                                SnackBar(content: Text(msg)),
                              );
                              Navigator.pop(context);
                            }
                          },
                          child: Padding(
                            padding: const EdgeInsets.all(16.0),
                            child: Text(tr(context, "submit")),
                          ),
                        ),
                      ),
                    ],
                  ),
                )),
          ],
        ));
  }
}
