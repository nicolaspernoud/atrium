import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:atrium/models/user.dart';
import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

class WhoAmIPopupWidget extends StatelessWidget {
  WhoAmIPopupWidget({super.key});

  final Uri _url = Uri.parse('https://github.com/nicolaspernoud/atrium');

  Future<void> _launchUrl() async {
    if (!await launchUrl(_url)) {
      throw Exception('Could not launch $_url');
    }
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<UserModel>(
      future: ApiProvider().whoAmI(),
      builder: (BuildContext context, AsyncSnapshot<UserModel> snapshot) {
        if (snapshot.connectionState == ConnectionState.waiting) {
          return const CircularProgressIndicator();
        } else if (snapshot.hasError) {
          return Text('Error: ${snapshot.error}');
        } else {
          UserModel user = snapshot.data!;
          return IconButton(
            icon: const Icon(Icons.person),
            onPressed: () {
              showDialog(
                context: context,
                builder: (BuildContext context) {
                  return AlertDialog(
                    title: const Center(
                        child: Padding(
                      padding: EdgeInsets.all(20.0),
                      child: Icon(
                        color: Colors.indigo,
                        Icons.person,
                        size: 70,
                      ),
                    )),
                    content: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text('${tr(context, "login")}: ${user.login}'),
                        Text(
                            '${tr(context, "roles")}: ${user.roles.join(", ")}'),
                        Text('${tr(context, "firstname")}: ${user.firstname}'),
                        Text('${tr(context, "lastname")}: ${user.lastname}'),
                        Text('${tr(context, "email")}: ${user.email}'),
                      ],
                    ),
                    actions: [
                      ButtonBar(
                          alignment: MainAxisAlignment.spaceBetween,
                          children: [
                            TextButton(
                              onPressed: _launchUrl,
                              child: Padding(
                                padding: const EdgeInsets.all(12.0),
                                child:
                                    Text('${tr(context, "powered_by")} atrium'),
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
                          ])
                    ],
                  );
                },
              );
            },
          );
        }
      },
    );
  }
}
