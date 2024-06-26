import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/user.dart';
import 'package:atrium/utils.dart';
import 'package:flutter/material.dart';

import '../i18n.dart';

class CreateEditUser extends StatefulWidget {
  final UserModel user;
  final bool isNew;
  const CreateEditUser({super.key, required this.user, required this.isNew});

  @override
  CreateEditUserState createState() => CreateEditUserState();
}

class CreateEditUserState extends State<CreateEditUser> {
  final _formKey = GlobalKey<FormState>();
  bool submitting = false;

  final _passController = TextEditingController();

  @override
  void initState() {
    super.initState();
    _passController.addListener(() {
      widget.user.password = _passController.text;
    });
    widget.user.password = "";
  }

  @override
  void dispose() {
    _passController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
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
              ? Text(tr(context, "edit_user"))
              : Text(tr(context, "new_user")),
          actions: !widget.isNew
              ? [
                  IconButton(
                      icon: const Icon(Icons.delete_forever),
                      onPressed: () async {
                        await ApiProvider().deleteUser(widget.user.login);
                        if (!context.mounted) return;
                        Navigator.pop(context);
                        ScaffoldMessenger.of(context).showSnackBar(SnackBar(
                            content: Text(MyLocalizations.of(context)!
                                .tr("user_deleted"))));
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
                        initialValue: widget.user.login,
                        decoration:
                            InputDecoration(labelText: tr(context, "login")),
                        validator: rejectEmpty,
                        onChanged: (value) {
                          widget.user.login = value;
                        },
                      ),
                      TextFormField(
                        controller: _passController,
                        decoration: InputDecoration(
                            labelText: tr(context, "password"),
                            suffixIcon: IconButton(
                                icon: const Icon(Icons.casino),
                                onPressed: () {
                                  _passController.text =
                                      generateRandomString(24);
                                }),
                            hintText: tr(context,
                                "leave_empty_to_keep_current_password")),
                        validator: widget.isNew ? rejectEmpty : null,
                      ),
                      TextFormField(
                        initialValue: widget.user.roles.join(","),
                        decoration:
                            InputDecoration(labelText: tr(context, "roles")),
                        validator: rejectEmpty,
                        onChanged: (value) {
                          widget.user.roles = value.split(",");
                        },
                      ),
                      TextFormField(
                        initialValue: widget.user.givenName,
                        decoration: InputDecoration(
                            labelText: tr(context, "given_name")),
                        onChanged: (value) {
                          widget.user.givenName = value;
                        },
                      ),
                      TextFormField(
                        initialValue: widget.user.familyName,
                        decoration: InputDecoration(
                            labelText: tr(context, "family_name")),
                        onChanged: (value) {
                          widget.user.familyName = value;
                        },
                      ),
                      TextFormField(
                        initialValue: widget.user.email,
                        decoration:
                            InputDecoration(labelText: tr(context, "email")),
                        onChanged: (value) {
                          widget.user.email = value;
                        },
                      ),
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 16.0),
                        child: !submitting
                            ? ElevatedButton(
                                onPressed: () async {
                                  // Validate returns true if the form is valid, or false otherwise.
                                  if (_formKey.currentState!.validate()) {
                                    var msg = tr(context, "user_created");
                                    try {
                                      setState(() {
                                        submitting = true;
                                      });
                                      await ApiProvider()
                                          .createUser(widget.user);
                                      // Do nothing on TypeError as Create respond with a null id
                                    } catch (e) {
                                      msg = e.toString();
                                    }
                                    if (!context.mounted) return;
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
                              )
                            : const Center(child: CircularProgressIndicator()),
                      ),
                    ],
                  ),
                )),
          ],
        ));
  }
}
