import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/app.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_colorpicker/flutter_colorpicker.dart';

import '../i18n.dart';
import 'icon_picker.dart';
import 'icons.dart';

class CreateEditApp extends StatefulWidget {
  final AppModel app;
  final bool isNew;
  const CreateEditApp({super.key, required this.app, required this.isNew});

  @override
  CreateEditAppState createState() => CreateEditAppState();
}

class CreateEditAppState extends State<CreateEditApp> {
  final _formKey = GlobalKey<FormState>();
  bool submitting = false;

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
                        if (!context.mounted) return;
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
                            icon: Icon(roundedIcons[widget.app.icon],
                                color: widget.app.color)),
                      ),
                    ),
                    Wrap(
                      children: [
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
                                    widget.app.insecureSkipVerify = false;
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
                        if (widget.app.isProxy) ...[
                          Row(
                            children: [
                              Checkbox(
                                value: widget.app.insecureSkipVerify,
                                onChanged: (bool? value) async {
                                  if (value != null) {
                                    widget.app.insecureSkipVerify = value;
                                    setState((() {}));
                                  }
                                },
                              ),
                              Padding(
                                padding: const EdgeInsets.all(16),
                                child:
                                    Text(tr(context, "insecure_skip_verify")),
                              ),
                            ],
                          )
                        ]
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
                      initialValue: widget.app.subdomains.join(","),
                      decoration:
                          InputDecoration(labelText: tr(context, "subdomains")),
                      onChanged: (value) {
                        widget.app.subdomains = value.split(",");
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
                    Row(
                      children: [
                        Checkbox(
                          value: widget.app.forwardUserMail,
                          onChanged: (bool? value) async {
                            if (value != null) {
                              widget.app.forwardUserMail = value;
                              setState((() {}));
                            }
                          },
                        ),
                        Flexible(
                          child: Padding(
                            padding: const EdgeInsets.all(16),
                            child: Text(tr(context, "forward_user_mail")),
                          ),
                        ),
                      ],
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
                                      var msg = tr(context, "app_created");
                                      try {
                                        setState(() {
                                          submitting = true;
                                        });
                                        await ApiProvider()
                                            .createApp(widget.app);
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
              ),
            ),
          ],
        ));
  }
}

Future<void> pickIcon(BuildContext context, Model model) async {
  model.icon = await showDialog<String>(
        context: context,
        barrierDismissible: true,
        builder: (BuildContext context) {
          return IconPicker(currentValue: model.icon);
        },
      ) ??
      model.icon;
  if (context.mounted) {
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
}
