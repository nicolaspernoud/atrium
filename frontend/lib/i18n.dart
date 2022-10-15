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
      "allow_symlinks": "Allow following symlinks",
      "apps": "Applications",
      "app_created": "Application created or altered with success",
      "cancel": "Cancel",
      "confirm_deletion_of": "Confirm deletion of",
      "copy": "Copy",
      "could_not_reach_server": "Could not reach server",
      "cpu_usage": "CPU usage",
      "cut": "Cut",
      "dav_created": "Webdav server created or altered with success",
      "days": "Days",
      "delete": "Delete",
      "download_success": "Download success",
      "download": "Download",
      "downloading": "Downloading...",
      "edit_app": "Edit app",
      "edit_dav": "Edit dav",
      "edit_user": "Edit user",
      "edit": "Edit",
      "email": "Email",
      "files": "Files",
      "firstname": "First name",
      "host": "Host",
      "hostname": "Hostname",
      "id": "Id",
      "inject_security_headers": "Inject security headers",
      "is_proxy": "Is a proxy",
      "is_writable": "Is writable",
      "lastname": "Last name",
      "leave_empty_to_keep_current_password":
          "Leave empty to keep current password",
      "login_screen": "Please log in...",
      "login": "Login",
      "login_failed": "Login failed",
      "memory_usage": "Memory usage",
      "name": "Name",
      "new_app": "New app",
      "new_dav": "New dav",
      "new_name": "New name",
      "new_user": "New user",
      "ok": "OK",
      "openpath": "Starting path opened in app",
      "open_in_new_tab": "Open in new tab",
      "passphrase": "Passphrase",
      "password": "Password",
      "pick_an_icon": "Pick an icon",
      "playback_speed": "Playback speed",
      "please_enter_some_text": "Please enter some text",
      "remove_dones": "Remove completed downloads",
      "rename": "Rename",
      "retry": "Retry",
      "roles": "Roles (separated by commas)",
      "secured": "Secure this application",
      "share_url_copied": "Share url copied to clipboard !",
      "share_with": "Share with",
      "share": "Share",
      "subdomains": "Subdomains (separated by commas)",
      "submit": "Submit",
      "system_information": "System info.",
      "target": "Target",
      "uploads": "Uploads",
      "uptime": "Uptime",
      "user_created": "User created or altered with success",
      "users": "Users",
      "close": "Close",
      "no_result_for": "No result for",
      "search": "Search",
    },
    'fr': {
      "allow_symlinks": "Autoriser le suivi des liens symboliques",
      "apps": "Applications",
      "app_created": "Application créée ou modifiée avec succès",
      "cancel": "Annuler",
      "close": "Fermer",
      "confirm_deletion_of": "Confirmer la suppression de",
      "copy": "Copier",
      "could_not_reach_server": "Impossible de joindre le serveur",
      "cpu_usage": "Utilisation CPU",
      "cut": "Couper",
      "dav_created": "Serveur webdav créé ou modifié avec succès",
      "days": "Jours",
      "delete": "Supprimer",
      "download_success": "Téléchargement terminé",
      "download": "Télécharger",
      "downloading": "Téléchargement...",
      "edit_app": "Modifier l'application",
      "edit_dav": "Modifier le dav",
      "edit_user": "Modifier l'utilisateur",
      "edit": "Modifier",
      "email": "Courriel",
      "files": "Fichiers",
      "firstname": "Prénom",
      "host": "Hôte",
      "hostname": "Nom d'hôte",
      "id": "Id",
      "inject_security_headers": "Injecter des en-têtes pour la sécurité",
      "is_proxy": "Serveur proxy",
      "is_writable": "Accès en écriture",
      "lastname": "Nom",
      "leave_empty_to_keep_current_password":
          "Laisser vide pour garder le mot de passe actuel",
      "login_screen": "Veuillez vous authentifier...",
      "login": "Nom d'utilisateur",
      "login_failed": "Erreur d'authentification",
      "memory_usage": "Utilisation mémoire",
      "name": "Nom",
      "new_app": "Nouvelle application",
      "new_dav": "Nouveau dav",
      "new_name": "Nouveau nom",
      "new_user": "Nouvel utilisateur",
      "no_result_for": "Aucun résultat pour",
      "ok": "OK",
      "open_in_new_tab": "Ouvrir dans un nouvel onglet",
      "openpath": "Chemin de démarrage de l'application",
      "passphrase": "Phrase de passe",
      "password": "Mot de passe",
      "pick_an_icon": "Choix de l'icône",
      "playback_speed": "Vitesse de lecture",
      "please_enter_some_text": "Merci d'entrer une chaîne de caractères",
      "remove_dones": "Enlever les téléchargement réussis",
      "rename": "Renommer",
      "retry": "Réessayer",
      "roles": "Rôles (séparés par des virgules)",
      "search": "Rechercher",
      "secured": "Sécuriser cette application",
      "share_url_copied": "Url de partage copiée dans le presse-papier !",
      "share_with": "Partager avec",
      "share": "Partager",
      "subdomains": "Sous domaines (séparés par des virgules)",
      "submit": "Valider",
      "system_information": "Info. système",
      "target": "Cible",
      "uploads": "Téléchargements",
      "uptime": "Temps de fonctionnement du serveur",
      "user_created": "Utilisateur créé ou modifié avec succès",
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
