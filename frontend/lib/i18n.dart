import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/foundation.dart' show SynchronousFuture;

String tr(context, String str) {
  return MyLocalizations.of(context)!.tr(str);
}

class MyLocalizations {
  MyLocalizations(this.locale);

  final Locale locale;

  static MyLocalizations? of(BuildContext context) {
    return Localizations.of<MyLocalizations>(context, MyLocalizations);
  }

  static final Map<String, Map<String, String>> _localizedValues = {
    'en': {
      "apps": "Apps",
      "submit": "Submit",
    },
    'fr': {
      "apps": "Applications",
      "submit": "Valider",
    },
  };

  String tr(String token) {
    return _localizedValues[locale.languageCode]![token] ?? token;
  }

  String formatDuration(Duration duration) {
    var dayLabel = "day";
    var hourLabel = "hour";
    var minuteLabel = "minute";
    var secondLabel = "second";

    if (locale.languageCode == 'fr') {
      dayLabel = "jour";
      hourLabel = "heure";
      secondLabel = "seconde";
    }

    var components = <String>[];

    var days = duration.inDays;
    if (days == 1) components.add('$days $dayLabel ');
    if (days > 1) components.add('$days ${dayLabel}s ');
    var hours = duration.inHours % 24;
    if (hours == 1) components.add('$hours $hourLabel ');
    if (hours > 1) components.add('$hours ${hourLabel}s ');
    var minutes = duration.inMinutes % 60;
    if (minutes == 1) components.add('$minutes $minuteLabel ');
    if (minutes > 1) components.add('$minutes ${minuteLabel}s ');

    var seconds = duration.inSeconds % 60;
    if (seconds == 1) components.add('$seconds $secondLabel');
    if (seconds > 1) components.add('$seconds ${secondLabel}s');

    return components.join();
  }
}

class MyLocalizationsDelegate extends LocalizationsDelegate<MyLocalizations> {
  const MyLocalizationsDelegate();

  @override
  bool isSupported(Locale locale) => ['en', 'fr'].contains(locale.languageCode);

  @override
  Future<MyLocalizations> load(Locale locale) {
    return SynchronousFuture<MyLocalizations>(MyLocalizations(locale));
  }

  @override
  bool shouldReload(MyLocalizationsDelegate old) => false;
}
