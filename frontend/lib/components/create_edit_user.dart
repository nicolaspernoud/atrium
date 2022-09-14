import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/user.dart';
import 'package:flutter/material.dart';

import '../i18n.dart';

class CreateEditUser extends StatefulWidget {
  final UserModel user;
  final bool isNew;
  const CreateEditUser({Key? key, required this.user, required this.isNew})
      : super(key: key);

  @override
  CreateEditUserState createState() => CreateEditUserState();
}

class CreateEditUserState extends State<CreateEditUser> {
  final _formKey = GlobalKey<FormState>();

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
                        if (!mounted) return;
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
                        initialValue: "",
                        decoration: InputDecoration(
                            labelText: tr(context, "password"),
                            hintText: tr(context,
                                "leave_empty_to_keep_current_password")),
                        validator: widget.isNew ? rejectEmpty : null,
                        onChanged: (value) {
                          widget.user.password = value;
                        },
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
                      Padding(
                        padding: const EdgeInsets.symmetric(vertical: 16.0),
                        child: ElevatedButton(
                          onPressed: () async {
                            // Validate returns true if the form is valid, or false otherwise.
                            if (_formKey.currentState!.validate()) {
                              var msg = tr(context, "user_created");
                              try {
                                await ApiProvider().createUser(widget.user);
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
