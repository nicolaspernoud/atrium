import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';

Future<void> showLoginDialog(BuildContext context, bool mounted) async {
  final formKey = GlobalKey<FormState>();
  await showDialog<String>(
    barrierDismissible: false,
    context: context,
    builder: (BuildContext context) =>
        LoginDialog(formKey: formKey, mounted: mounted),
  );
}

class LoginDialog extends StatefulWidget {
  const LoginDialog({
    super.key,
    required this.formKey,
    required this.mounted,
  });

  final GlobalKey<FormState> formKey;

  final bool mounted;

  @override
  State<LoginDialog> createState() => _LoginDialogState();
}

class _LoginDialogState extends State<LoginDialog> {
  String login = "";
  String password = "";
  bool _isObscure = true;
  String errorMessage = "";

  @override
  Widget build(BuildContext context) {
    if (kIsWeb && !kDebugMode) {
      App().prefs.hostname = Uri.base.origin;
    }
    return AlertDialog(
      title: Text(tr(context, "login_screen")),
      content: SizedBox(
        height: 250,
        width: 350,
        child: Form(
          key: widget.formKey,
          child: Column(
            children: [
              if (!kIsWeb || kDebugMode)
                TextFormField(
                  initialValue: App().prefs.hostname,
                  decoration:
                      InputDecoration(labelText: tr(context, "hostname")),
                  onChanged: (text) {
                    setState(() {
                      App().prefs.hostname = text;
                    });
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
                autofillHints: const [AutofillHints.username],
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
                obscureText: _isObscure,
                initialValue: password,
                autofillHints: const [AutofillHints.password],
                decoration: InputDecoration(
                    labelText: tr(context, "password"),
                    // this button is used to toggle the password visibility
                    suffixIcon: IconButton(
                        icon: Icon(_isObscure
                            ? Icons.visibility
                            : Icons.visibility_off),
                        onPressed: () {
                          setState(() {
                            _isObscure = !_isObscure;
                          });
                        })),
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
                onFieldSubmitted: (value) => submitForm(),
              ),
              Expanded(
                child: Container(),
              ),
              Text(
                  style: const TextStyle(
                      fontWeight: FontWeight.bold, color: Colors.red),
                  errorMessage)
            ],
          ),
        ),
      ),
      actions: [
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          children: [
            FutureBuilder(
              future: ApiProvider().hasOIDC(),
              builder: (BuildContext context, AsyncSnapshot<bool> snapshot) {
                Widget child;
                if (snapshot.hasData && snapshot.data == true) {
                  child = TextButton.icon(
                    icon: const Icon(Icons.login), // Your icon here
                    label: const Padding(
                      padding: EdgeInsets.all(12.0),
                      child: Text("OpenID Connect"),
                    ), // Your text here
                    onPressed: () {
                      openIdConnectLogin(context);
                      // If there is an ATRIUM_REDIRECT cookie set, redirect to the target
                      if (kIsWeb) {
                        redirectToAppAfterAuth();
                      }
                    },
                  );
                } else {
                  child = Container();
                }
                return AnimatedSwitcher(
                  duration: const Duration(milliseconds: 250),
                  child: child,
                );
              },
            ),
            TextButton.icon(
                icon: const Icon(Icons.check_rounded), // Your icon here
                label: const Padding(
                  padding: EdgeInsets.all(12.0),
                  child: Text("OK"),
                ), // Your text here
                onPressed: submitForm)
          ],
        ),
      ],
    );
  }

  Future<void> submitForm() async {
    if (widget.formKey.currentState!.validate()) {
      try {
        await ApiProvider().login(login, password);
        if (!widget.mounted) return;
        Navigator.pop(context, 'OK');
        // If there is an ATRIUM_REDIRECT cookie set, redirect to the target
        if (kIsWeb) {
          redirectToAppAfterAuth();
        }
      } catch (e) {
        if (e is DioException) {
          if (e.response?.statusCode == 401) {
            setState(() {
              errorMessage = tr(context, "login_failed");
            });
          } else {
            setState(() {
              errorMessage = tr(context, "could_not_reach_server");
            });
          }
          await Future.delayed(const Duration(seconds: 3));
          if (mounted) {
            setState(() {
              errorMessage = "";
            });
          }
        }
      }
    }
  }
}
