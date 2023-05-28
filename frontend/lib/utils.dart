import 'dart:math';

import 'package:atrium/components/whoami_popup.dart';
import 'package:flutter/material.dart';
import 'globals.dart';

Color colorFromPercent(double? percent) {
  if (percent == null) return Colors.grey;
  if (percent > 0.80) return Colors.red;
  if (percent > 0.70) return Colors.orange;
  if (percent > 0.60) return Colors.yellow;
  return Colors.green;
}

List<Widget> logoutAction = <Widget>[
  Row(
    children: [
      WhoAmIPopupWidget(),
      IconButton(
        icon: const Icon(Icons.logout),
        onPressed: () {
          App().cookie = "";
        },
      ),
    ],
  ),
];

String generateRandomString(int length) {
  final random = Random();
  const availableChars =
      'AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz1234567890';
  return List.generate(length,
      (index) => availableChars[random.nextInt(availableChars.length)]).join();
}
