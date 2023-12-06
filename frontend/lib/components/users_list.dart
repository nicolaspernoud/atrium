import 'package:atrium/components/create_edit_user.dart';
import 'package:atrium/components/delete_dialog.dart';
import 'package:atrium/components/login_dialog.dart';
import 'package:atrium/components/user_dialog.dart';
import 'package:atrium/i18n.dart';
import 'package:atrium/models/api_provider.dart';
import 'package:dio/dio.dart';
import 'package:flutter/material.dart';

import '../models/user.dart';

class UsersList extends StatefulWidget {
  const UsersList({super.key});

  @override
  State<UsersList> createState() => _UsersListState();
}

class _UsersListState extends State<UsersList> {
  Future<void> openLoginDialog(_) async {
    await showLoginDialog(context, mounted);
  }

  late Future<List<UserModel>> users;

  @override
  void initState() {
    super.initState();
    _getData();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Row(
          children: [
            const Icon(
              Icons.group,
              size: 30,
            ),
            const SizedBox(width: 15),
            Text(tr(context, "users")),
          ],
        ),
        actions: const [UserDialogOpener()],
      ),
      body: FutureBuilder(
          future: users,
          builder:
              (BuildContext context, AsyncSnapshot<List<UserModel>> snapshot) {
            switch (snapshot.connectionState) {
              case ConnectionState.none:
              case ConnectionState.active:
              case ConnectionState.waiting:
                return const Center(child: CircularProgressIndicator());
              case ConnectionState.done:
                if (snapshot.hasError &&
                    snapshot.error is DioException &&
                    (snapshot.error as DioException).response?.statusCode ==
                        401) {
                  // If error is 401, we log and retry
                  Future.delayed(Duration.zero, () async {
                    await showLoginDialog(context, mounted);
                    await _getData();
                    setState(() {});
                  });
                  return const Center(child: CircularProgressIndicator());
                }
                if (snapshot.hasError) {
                  return Center(child: Text('Error: ${snapshot.error}'));
                }
                return _buildListView(context, snapshot.data ?? []);
            }
          }),
      floatingActionButton: FloatingActionButton.small(
          child: const Icon(Icons.add),
          onPressed: () async {
            var user = UserModel();
            await Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) => CreateEditUser(user: user, isNew: true),
                ));
            await _getData();
            setState(() {});
          }),
    );
  }

  _getData() {
    users = ApiProvider().getUsers();
  }

  Widget _buildListView(BuildContext context, List<UserModel> list) {
    return ListView.builder(
        padding: const EdgeInsets.all(8.0),
        itemCount: list.length,
        itemBuilder: (context, index) {
          final user = list[index];
          return ListTile(
            leading: const Icon(Icons.account_circle),
            title: Text(user.login),
            subtitle: Text(user.roles.join(",")),
            trailing: user.isDeleting
                ? const DeletingSpinner()
                : Row(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      IconButton(
                          icon: const Icon(Icons.edit),
                          onPressed: () async {
                            await Navigator.push(
                                context,
                                MaterialPageRoute(
                                  builder: (context) =>
                                      CreateEditUser(user: user, isNew: false),
                                ));
                            await _getData();
                            setState(() {});
                          }),
                      IconButton(
                          icon: const Icon(Icons.delete_forever),
                          onPressed: () async {
                            var confirmed = await showDialog<bool>(
                              context: context,
                              builder: (context) => DeleteDialog(user.login),
                            );
                            if (confirmed!) {
                              setState(() {
                                user.isDeleting = true;
                              });
                              await ApiProvider().deleteUser(user.login);
                              await _getData();
                              setState(() {});
                            }
                          }),
                    ],
                  ),
          );
        });
  }
}
