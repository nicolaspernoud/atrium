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
  IconButton(
    icon: const Icon(Icons.logout),
    onPressed: () {
      App().cookie = "";
    },
  ),
];
