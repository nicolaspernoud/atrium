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
      "app_created": "Application created or altered with success",
      "apps": "Applications",
      "at_least_3_chars": "Please enter at least 3 characters...",
      "cancel": "Cancel",
      "close": "Close",
      "confirm_deletion_of": "Confirm deletion of",
      "copy": "Copy",
      "could_not_reach_server": "Could not reach server",
      "cpu_usage": "CPU usage",
      "cut": "Cut",
      "dates": "Dates",
      "dav_created": "Webdav server created or altered with success",
      "days": "Days",
      "delete": "Delete",
      "do_not_zip_folder": "Generate HTML links instead of zipping the folder",
      "download_success": "Download success",
      "download": "Download",
      "downloading": "Downloading...",
      "edit_app": "Edit app",
      "edit_dav": "Edit dav",
      "edit_user": "Edit user",
      "edit": "Edit",
      "email": "Email",
      "failed_share_token": "Failed to generate a share token",
      "files": "Files",
      "firstname": "First name",
      "forward_user_mail":
          "Forward authenticated user email to the proxied app using the Remote-User header",
      "host": "Host",
      "hostname": "Hostname",
      "id": "Id",
      "inject_security_headers": "Inject security headers",
      "is_proxy": "Is a proxy",
      "is_writable": "Is writable",
      "lastname": "Last name",
      "leave_empty_to_keep_current_password":
          "Leave empty to keep current password",
      "login_failed": "Login failed",
      "login_screen": "Please log in...",
      "login": "Login",
      "logout": "Logout",
      "memory_usage": "Memory usage",
      "name": "Name",
      "names": "Names",
      "new_app": "New app",
      "new_dav": "New dav",
      "new_name": "New name",
      "new_user": "New user",
      "ok": "OK",
      "open_in_new_tab": "Open in new tab",
      "openpath": "Starting path opened in app",
      "passphrase": "Passphrase",
      "password": "Password",
      "pick_an_icon": "Pick an icon",
      "playback_speed": "Playback speed",
      "please_enter_some_text": "Please enter some text",
      "powered_by": "Powered by",
      "remove_dones": "Remove completed downloads",
      "rename": "Rename",
      "retry": "Retry",
      "roles": "Roles (separated by commas)",
      "search": "Search",
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
      "user": "User",
      "users": "Users",
    },
    'fr': {
      "allow_symlinks": "Autoriser le suivi des liens symboliques",
      "app_created": "Application créée ou modifiée avec succès",
      "apps": "Applications",
      "at_least_3_chars": "Veuillez entrer au minimum 3 caractères...",
      "cancel": "Annuler",
      "close": "Fermer",
      "confirm_deletion_of": "Confirmer la suppression de",
      "copy": "Copier",
      "could_not_reach_server": "Impossible de joindre le serveur",
      "cpu_usage": "Utilisation CPU",
      "cut": "Couper",
      "dates": "Dates",
      "dav_created": "Serveur webdav créé ou modifié avec succès",
      "days": "Jours",
      "delete": "Supprimer",
      "do_not_zip_folder":
          "Générer des liens HTML au lieu de compresser le dossier",
      "download_success": "Téléchargement terminé",
      "download": "Télécharger",
      "downloading": "Téléchargement...",
      "edit_app": "Modifier l'application",
      "edit_dav": "Modifier le dav",
      "edit_user": "Modifier l'utilisateur",
      "edit": "Modifier",
      "email": "Courriel",
      "failed_share_token": "Erreur lors de la génération du jeton de partage",
      "files": "Fichiers",
      "firstname": "Prénom",
      "forward_user_mail":
          "Transférer le mail de l'utilisateur connecté à l'application cible via l'entête Remote-User",
      "host": "Hôte",
      "hostname": "Nom d'hôte",
      "id": "Id",
      "inject_security_headers": "Injecter des en-têtes pour la sécurité",
      "is_proxy": "Serveur proxy",
      "is_writable": "Accès en écriture",
      "lastname": "Nom",
      "leave_empty_to_keep_current_password":
          "Laisser vide pour garder le mot de passe actuel",
      "login_failed": "Erreur d'authentification",
      "login_screen": "Veuillez vous authentifier...",
      "login": "Nom d'utilisateur",
      "logout": "Déconnexion",
      "memory_usage": "Utilisation mémoire",
      "name": "Nom",
      "names": "Noms",
      "new_app": "Nouvelle application",
      "new_dav": "Nouveau dav",
      "new_name": "Nouveau nom",
      "new_user": "Nouvel utilisateur",
      "ok": "OK",
      "open_in_new_tab": "Ouvrir dans un nouvel onglet",
      "openpath": "Chemin de démarrage de l'application",
      "passphrase": "Phrase de passe",
      "password": "Mot de passe",
      "pick_an_icon": "Choix de l'icône",
      "playback_speed": "Vitesse de lecture",
      "please_enter_some_text": "Merci d'entrer une chaîne de caractères",
      "powered_by": "Propulsé par",
      "remove_dones": "Masquer les téléchargement réussis",
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
      "user": "Utilisateur",
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
