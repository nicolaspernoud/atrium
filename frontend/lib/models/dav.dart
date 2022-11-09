import 'package:atrium/models/app.dart';
import 'package:flutter/widgets.dart';

class DavModel implements Model {
  DavModel({
    required this.id,
    this.host = "",
    this.directory = "/tmp",
    this.writable = true,
    this.name = "",
    this.icon = "folder",
    this.color = const Color(0xffd32f2f),
    this.secured = false,
    this.allowSymlinks = false,
    this.roles = const ["ADMINS", "USERS"],
    this.passphrase = "",
  });

  late int id;
  @override
  late String host;
  late String directory;
  late bool writable;
  late String name;
  @override
  late String icon;
  @override
  late Color color;
  late bool secured;
  late bool allowSymlinks;
  late List<String> roles;
  late String? passphrase;

  DavModel.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    host = json['host'];
    directory = json['directory'];
    writable = json['writable'] ?? false;
    name = json['name'];
    icon = json['icon'] != "" ? json['icon'] : "folder";
    color = Color(json['color']);
    secured = json['secured'] ?? false;
    allowSymlinks = json['allow_symlinks'] ?? false;
    roles = json['roles'] != null
        ? List.castFrom<dynamic, String>(json['roles'])
        : [];
    passphrase = json['passphrase'] ?? "";
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['id'] = id;
    data['host'] = host;
    data['directory'] = directory;
    data['writable'] = writable;
    data['name'] = name;
    data['icon'] = icon;
    data['color'] = color.value;
    data['secured'] = secured;
    data['allow_symlinks'] = allowSymlinks;
    data['roles'] = roles;
    data['passphrase'] = passphrase;
    return data;
  }
}
