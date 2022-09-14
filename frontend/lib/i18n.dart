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
      "apps": "Applications",
      "app_created": "Application created or altered with success",
      "cancel": "Cancel",
      "copy": "Copy",
      "cpu_usage": "CPU usage",
      "cut": "Cut",
      "dav_created": "Dav created or altered with success",
      "davs": "Davs",
      "days": "Days",
      "delete": "Delete",
      "download": "Download",
      "edit_app": "Edit app",
      "edit_dav": "Edit dav",
      "edit_user": "Edit user",
      "edit": "Edit",
      "files": "Files",
      "host": "Host",
      "hostname": "Hostname",
      "id": "Id",
      "is_proxy": "Is a proxy",
      "is_writable": "Is writable",
      "login": "Login",
      "memory_usage": "Memory usage",
      "name": "Name",
      "new_app": "New app",
      "new_dav": "New dav",
      "new_name": "New name",
      "new_user": "New user",
      "ok": "OK",
      "openpath": "Starting path opened in app",
      "passphrase": "Passphrase",
      "password": "Password",
      "pick_an_icon": "Pick an icon",
      "please_enter_some_text": "Please enter some text",
      "remove_dones": "Remove completed downloads",
      "rename": "Rename",
      "retry": "Retry",
      "roles": "Roles",
      "secured": "Secure this application",
      "share_url_copied": "Share url copied to clipboard !",
      "share_with": "Share with",
      "share": "Share",
      "submit": "Submit",
      "system_information": "System info.",
      "target": "Target",
      "uploads": "Uploads",
      "uptime": "Uptime",
      "user_created": "User created",
      "users": "Users",
      "close": "Close",
      "no_result_for": "No result for",
      "search": "Search",
    },
    'fr': {
      "apps": "Applications",
      "app_created": "Application créée ou modifiée avec succès",
      "cancel": "Annuler",
      "close": "Fermer",
      "copy": "Copy",
      "cpu_usage": "Utilisation CPU",
      "cut": "Couper",
      "dav_created": "Dav créé ou modifié avec succès",
      "davs": "Davs",
      "days": "Jours",
      "delete": "Supprimer",
      "download": "Télécharger",
      "edit_app": "Modifier l'application",
      "edit_dav": "Modifier le dav",
      "edit_user": "Modifier l'utilisateur",
      "edit": "Modifier",
      "files": "Fichiers",
      "host": "Hôte",
      "hostname": "Nom d'hôte",
      "id": "Id",
      "is_proxy": "Serveur proxy",
      "is_writable": "Accès en écriture",
      "login": "Nom d'utilisateur",
      "memory_usage": "Utilisation mémoire",
      "name": "Nom",
      "new_app": "Nouvelle application",
      "new_dav": "Nouveau dav",
      "new_name": "Nouveau nom",
      "new_user": "Nouvel utilisateur",
      "no_result_for": "Aucun résultat pour",
      "ok": "OK",
      "openpath": "Chemin de démarrage de l'application",
      "passphrase": "Phrase de passe",
      "password": "Mot de passe",
      "pick_an_icon": "Choix de l'icône",
      "please_enter_some_text": "Merci d'entrer une chaîne de caractères",
      "remove_dones": "Enlever les téléchargement réussis",
      "rename": "Renommer",
      "retry": "Réessayer",
      "roles": "Rôles",
      "search": "Rechercher",
      "secured": "Sécuriser cette application",
      "share_url_copied": "Url de partage copiée dans le presse-papier !",
      "share_with": "Partager avec",
      "share": "Partager",
      "submit": "Valider",
      "system_information": "Info. système",
      "target": "Serveur cible",
      "uploads": "Téléchargements",
      "uptime": "Temps de fonctionnement du serveur",
      "user_created": "Utilisateur créé",
      "users": "Utilisateurs",
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
