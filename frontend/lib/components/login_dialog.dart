import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/utils.dart';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:atrium/platform/mobile.dart'
    if (dart.library.html) 'package:atrium/platform/web.dart';
import 'package:flutter/services.dart';

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
  const LoginDialog({super.key, required this.formKey, required this.mounted});

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
    final compactY = isSmallVertical(context);
    final screenWidth = MediaQuery.of(context).size.width;
    final dialogWidth =
        compactY ? (screenWidth * 0.95).clamp(350.0, 750.0) : 350.0;

    final hostnameField = TextFormField(
      initialValue: App().prefs.hostname,
      decoration: InputDecoration(
        labelText: tr(context, "hostname"),
        isDense: compactY,
      ),
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
    );

    final loginField = TextFormField(
      initialValue: login,
      autofillHints: const [AutofillHints.username],
      decoration: InputDecoration(
        labelText: tr(context, "login"),
        isDense: compactY,
      ),
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
    );

    final passwordField = TextFormField(
      obscureText: _isObscure,
      initialValue: password,
      autofillHints: const [AutofillHints.password],
      decoration: InputDecoration(
        labelText: tr(context, "password"),
        isDense: compactY,
        suffixIcon: IconButton(
          icon: Icon(
            _isObscure ? Icons.visibility : Icons.visibility_off,
          ),
          onPressed: () {
            setState(() {
              _isObscure = !_isObscure;
            });
          },
        ),
      ),
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
    );

    final okButton = TextButton.icon(
      icon: const Icon(Icons.check_rounded),
      label: const Padding(
        padding: EdgeInsets.all(12.0),
        child: Text("OK"),
      ),
      onPressed: submitForm,
    );

    final oidcWidget = FutureBuilder(
      future: ApiProvider().hasOIDC(),
      builder: (BuildContext context, AsyncSnapshot<bool> snapshot) {
        Widget child;
        if (snapshot.hasData && snapshot.data == true) {
          child = TextButton.icon(
            icon: const Icon(Icons.login),
            label: const Padding(
              padding: EdgeInsets.all(12.0),
              child: Text("OpenID Connect"),
            ),
            onPressed: () {
              openIdConnectLogin(context);
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
    );

    return AlertDialog(
      title: Text(tr(context, "login_screen")),
      content: SizedBox(
        width: dialogWidth,
        child: Form(
          key: widget.formKey,
          child: compactY
              ? SingleChildScrollView(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Row(
                        crossAxisAlignment: CrossAxisAlignment.center,
                        children: [
                          if (!kIsWeb || kDebugMode) ...[
                            Expanded(child: hostnameField),
                            const SizedBox(width: 8),
                          ],
                          Expanded(
                            flex: 2,
                            child: AutofillGroup(
                              child: Row(
                                children: [
                                  Expanded(child: loginField),
                                  const SizedBox(width: 8),
                                  Expanded(child: passwordField),
                                ],
                              ),
                            ),
                          ),
                          const SizedBox(width: 8),
                          okButton,
                          const SizedBox(width: 8),
                          oidcWidget,
                        ],
                      ),
                      if (errorMessage.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.only(top: 4.0),
                          child: Text(
                            errorMessage,
                            style: const TextStyle(
                              fontWeight: FontWeight.bold,
                              color: Colors.red,
                            ),
                          ),
                        ),
                    ],
                  ),
                )
              : SingleChildScrollView(
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      if (!kIsWeb || kDebugMode) hostnameField,
                      AutofillGroup(
                        child: Column(
                          children: [
                            loginField,
                            passwordField,
                          ],
                        ),
                      ),
                      if (errorMessage.isNotEmpty)
                        Padding(
                          padding: const EdgeInsets.only(top: 8.0),
                          child: Text(
                            errorMessage,
                            style: const TextStyle(
                              fontWeight: FontWeight.bold,
                              color: Colors.red,
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
        ),
      ),
      actions: compactY
          ? null
          : [
              Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  oidcWidget,
                  okButton,
                ],
              ),
            ],
    );
  }

  Future<void> submitForm() async {
    if (widget.formKey.currentState!.validate()) {
      TextInput.finishAutofillContext();
      try {
        await ApiProvider().login(login, password);
        if (!mounted) return;
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
