import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';

Future<void> showLoginDialog(BuildContext context, bool mounted) async {
  final formKey = GlobalKey<FormState>();
  while (!App().hasToken) {
    await showDialog<String>(
      context: context,
      builder: (BuildContext context) =>
          LoginDialog(formKey: formKey, mounted: mounted),
    );
  }
}

class LoginDialog extends StatefulWidget {
  const LoginDialog({
    Key? key,
    required this.formKey,
    required this.mounted,
  }) : super(key: key);

  final GlobalKey<FormState> formKey;

  final bool mounted;

  @override
  State<LoginDialog> createState() => _LoginDialogState();
}

class _LoginDialogState extends State<LoginDialog> {
  String login = "admin";
  String password = "password";
  @override
  Widget build(BuildContext context) {
    if (kIsWeb && !kDebugMode) {
      App().prefs.hostname = Uri.base.origin;
    }
    return AlertDialog(
      title: Text(tr(context, "login")),
      content: SizedBox(
        height: 250,
        child: Form(
          key: widget.formKey,
          child: Column(
            children: [
              if (!kIsWeb || kDebugMode)
                TextFormField(
                  //initialValue: App().prefs.hostname,
                  initialValue: App().prefs.hostname != ""
                      ? App().prefs.hostname
                      : "http://atrium.127.0.0.1.nip.io:8080-",
                  decoration:
                      InputDecoration(labelText: tr(context, "hostname")),
                  onChanged: (text) {
                    App().prefs.hostname = text;
                  },
                  validator: (value) {
                    if (value == null || value.isEmpty) {
                      return tr(context, "please_enter_some_text");
                    }
                    return null;
                  },
                  key: const Key("hostnameField"),
                ),
              TextFormField(
                initialValue: login,
                decoration: InputDecoration(labelText: tr(context, "login")),
                key: const Key("loginField"),
                onChanged: (text) {
                  login = text;
                },
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return tr(context, "please_enter_some_text");
                  }
                  return null;
                },
              ),
              TextFormField(
                initialValue: password,
                decoration: InputDecoration(labelText: tr(context, "password")),
                key: const Key("userPasswordField"),
                onChanged: (text) {
                  password = text;
                },
                validator: (value) {
                  if (value == null || value.isEmpty) {
                    return tr(context, "please_enter_some_text");
                  }
                  return null;
                },
              ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () async {
            if (widget.formKey.currentState!.validate()) {
              await ApiProvider().login(login, password);
              if (!widget.mounted) return;
              Navigator.pop(context, 'OK');
            }
          },
          child: const Text('OK'),
        ),
      ],
    );
  }
}
