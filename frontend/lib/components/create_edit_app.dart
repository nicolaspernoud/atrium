import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/app.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_colorpicker/flutter_colorpicker.dart';
import 'package:flutter_iconpicker/flutter_iconpicker.dart';

import '../i18n.dart';

class CreateEditApp extends StatefulWidget {
  final AppModel app;
  final bool isNew;
  const CreateEditApp({Key? key, required this.app, required this.isNew})
      : super(key: key);

  @override
  CreateEditAppState createState() => CreateEditAppState();
}

class CreateEditAppState extends State<CreateEditApp> {
  final _formKey = GlobalKey<FormState>();

  @override
  Widget build(BuildContext context) {
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
              ? Text(tr(context, "edit_app"))
              : Text(tr(context, "new_app")),
          actions: !widget.isNew
              ? [
                  IconButton(
                      icon: const Icon(Icons.delete_forever),
                      onPressed: () async {
                        await ApiProvider().deleteApp(widget.app.id);
                        if (!mounted) return;
                        Navigator.pop(context);
                        ScaffoldMessenger.of(context).showSnackBar(SnackBar(
                            content: Text(MyLocalizations.of(context)!
                                .tr("app_deleted"))));
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
                      initialValue: widget.app.id.toString(),
                      decoration: InputDecoration(labelText: tr(context, "id")),
                      keyboardType: TextInputType.number,
                      inputFormatters: [intOnly],
                      validator: rejectEmpty,
                      onChanged: (value) {
                        widget.app.id = int.parse(value);
                      },
                    ),
                    TextFormField(
                      initialValue: widget.app.name,
                      decoration:
                          InputDecoration(labelText: tr(context, "name")),
                      validator: rejectEmpty,
                      onChanged: (value) {
                        widget.app.name = value;
                      },
                    ),
                    Center(
                      child: Padding(
                        padding: const EdgeInsets.all(16.0),
                        child: IconButton(
                            iconSize: 40.0,
                            onPressed: () async {
                              await pickIcon(context, widget.app);
                              setState(() {});
                            },
                            icon: Icon(
                                IconData(widget.app.icon,
                                    fontFamily: 'MaterialIcons'),
                                color: widget.app.color)),
                      ),
                    ),
                    Row(
                      children: [
                        Checkbox(
                          value: widget.app.isProxy,
                          onChanged: (bool? value) async {
                            if (value != null) {
                              widget.app.isProxy = value;
                              if (value == false) {
                                widget.app.login = "";
                                widget.app.password = "";
                                widget.app.openpath = "";
                              }
                              setState((() {}));
                            }
                          },
                        ),
                        Padding(
                          padding: const EdgeInsets.all(16),
                          child: Text(tr(context, "is_proxy")),
                        ),
                      ],
                    ),
                    TextFormField(
                      initialValue: widget.app.host,
                      decoration:
                          InputDecoration(labelText: tr(context, "host")),
                      validator: rejectEmpty,
                      onChanged: (value) {
                        widget.app.host = value;
                      },
                    ),
                    TextFormField(
                      initialValue: widget.app.target,
                      decoration:
                          InputDecoration(labelText: tr(context, "target")),
                      validator: rejectEmpty,
                      onChanged: (value) {
                        widget.app.target = value;
                      },
                    ),
                    Row(
                      children: [
                        Checkbox(
                          value: widget.app.secured,
                          onChanged: (bool? value) async {
                            if (value != null) {
                              widget.app.secured = value;
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
                    if (widget.app.isProxy) ...[
                      TextFormField(
                        initialValue: widget.app.login,
                        decoration:
                            InputDecoration(labelText: tr(context, "login")),
                        onChanged: (value) {
                          widget.app.login = value;
                        },
                      ),
                      TextFormField(
                        initialValue: widget.app.password,
                        decoration:
                            InputDecoration(labelText: tr(context, "password")),
                        onChanged: (value) {
                          widget.app.password = value;
                        },
                      ),
                      TextFormField(
                        initialValue: widget.app.openpath,
                        decoration:
                            InputDecoration(labelText: tr(context, "openpath")),
                        onChanged: (value) {
                          widget.app.openpath = value;
                        },
                      ),
                    ],
                    TextFormField(
                      initialValue: widget.app.roles.join(","),
                      decoration:
                          InputDecoration(labelText: tr(context, "roles")),
                      validator: rejectEmpty,
                      onChanged: (value) {
                        widget.app.roles = value.split(",");
                      },
                    ),
                    Row(
                      children: [
                        Checkbox(
                          value: widget.app.injectSecurityHeaders,
                          onChanged: (bool? value) async {
                            if (value != null) {
                              widget.app.injectSecurityHeaders = value;
                              setState((() {}));
                            }
                          },
                        ),
                        Padding(
                          padding: const EdgeInsets.all(16),
                          child: Text(tr(context, "inject_security_headers")),
                        ),
                      ],
                    ),
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 16.0),
                      child: ElevatedButton(
                        onPressed: () async {
                          // Validate returns true if the form is valid, or false otherwise.
                          if (_formKey.currentState!.validate()) {
                            var msg = tr(context, "app_created");
                            try {
                              await ApiProvider().createApp(widget.app);
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
              ),
            ),
          ],
        ));
  }
}

pickIcon(BuildContext context, Model model) async {
  IconData? icon = await FlutterIconPicker.showIconPicker(context,
      title: Text(tr(context, 'pick_an_icon')),
      searchHintText: tr(context, 'search'),
      noResultsText: tr(context, 'no_result_for'),
      closeChild: Text(
        tr(context, 'close'),
        textScaleFactor: 1.25,
      ));
  model.icon = icon!.codePoint;
  await showDialog(
    context: context,
    builder: (BuildContext context) {
      return AlertDialog(
        titlePadding: const EdgeInsets.all(0),
        contentPadding: const EdgeInsets.all(16),
        content: SingleChildScrollView(
          child: MaterialPicker(
            pickerColor: model.color,
            onColorChanged: (color) {
              model.color = color;
              Navigator.pop(context, 'OK');
            },
            enableLabel: true,
            portraitOnly: true,
          ),
        ),
      );
    },
  );
}
