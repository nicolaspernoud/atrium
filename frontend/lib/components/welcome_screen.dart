import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/globals.dart';
import 'package:flutter/material.dart';

class WelcomeScreen extends StatelessWidget {
  const WelcomeScreen({Key? key}) : super(key: key);

  @override
  Widget build(BuildContext context) {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!App().hasToken) {
        showLoginDialog(context, true);
      }
    });

    return const Center(child: Text("Please log in..."));
  }
}
