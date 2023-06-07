import 'package:atrium/globals.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/user.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

class UserDialogOpener extends StatefulWidget {
  const UserDialogOpener({Key? key}) : super(key: key);

  @override
  UserDialogOpenerState createState() => UserDialogOpenerState();
}

class UserDialogOpenerState extends State<UserDialogOpener> {
  final Uri _url = Uri.parse('https://github.com/nicolaspernoud/atrium');

  Future<void> _launchUrl() async {
    if (!await launchUrl(_url)) {
      throw Exception('Could not launch $_url');
    }
  }

  @override
  Widget build(BuildContext context) {
    return IconButton(
      icon: const Icon(Icons.person),
      onPressed: () {
        showDialog(
          context: context,
          builder: (BuildContext context) {
            return AlertDialog(
              icon: const Icon(
                color: Colors.indigo,
                Icons.person,
                size: 70,
              ),
              content: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  FutureBuilder<UserModel>(
                    future: ApiProvider().whoAmI(),
                    builder: (BuildContext context,
                        AsyncSnapshot<UserModel> snapshot) {
                      return AnimatedSwitcher(
                        duration: const Duration(milliseconds: 500),
                        child: _buildContent(context, snapshot),
                      );
                    },
                  ),
                  const SizedBox(height: 40.0),
                  TextButton(
                    onPressed: _launchUrl,
                    child: Padding(
                      padding: const EdgeInsets.all(8.0),
                      child: Text('${tr(context, "powered_by")} atrium'),
                    ),
                  ),
                ],
              ),
              actionsAlignment: MainAxisAlignment.spaceBetween,
              actions: [
                TextButton.icon(
                  onPressed: () {
                    Navigator.of(context).pop();
                    App().cookie = "";
                  },
                  icon: const Icon(
                    Icons.logout,
                  ),
                  label: Padding(
                    padding: const EdgeInsets.only(
                        right: 12.0, top: 12.0, bottom: 12.0),
                    child: Text(tr(context, "logout")),
                  ),
                ),
                TextButton(
                  onPressed: () {
                    Navigator.of(context).pop();
                  },
                  child: Padding(
                    padding: const EdgeInsets.all(12.0),
                    child: Text(tr(context, "close")),
                  ),
                ),
              ],
            );
          },
        );
      },
    );
  }

  Widget _buildContent(
      BuildContext context, AsyncSnapshot<UserModel> snapshot) {
    if (snapshot.connectionState == ConnectionState.waiting) {
      return const SizedBox(
        width: 60,
        height: 60,
        child: Center(child: CircularProgressIndicator()),
      );
    } else if (snapshot.hasError) {
      return Text('Error: ${snapshot.error}');
    } else {
      UserModel user = snapshot.data!;
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text('${tr(context, "login")}: ${user.login}'),
          Text('${tr(context, "roles")}: ${user.roles.join(", ")}'),
          Text('${tr(context, "firstname")}: ${user.firstname}'),
          Text('${tr(context, "lastname")}: ${user.lastname}'),
          Text('${tr(context, "email")}: ${user.email}'),
        ],
      );
    }
  }
}
