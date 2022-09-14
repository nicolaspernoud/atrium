import 'package:atrium/models/app.dart';
import 'package:flutter/widgets.dart';

class DavModel implements Model {
  DavModel({
    required this.id,
    this.host = "",
    this.directory = "/tmp",
    this.writable = true,
    this.name = "",
    this.icon = 0xf0330,
    this.color = const Color.fromARGB(0, 20, 224, 37),
    this.secured = false,
    this.allowSymlinks = false,
    this.roles = const ["ADMINS", "USERS"],
    this.passphrase = "",
  });

  late int id;
  late String host;
  late String directory;
  late bool writable;
  late String name;
  @override
  late int icon;
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
    writable = json['writable'];
    name = json['name'];
    icon = json['icon'];
    color = Color(json['color']);
    secured = json['secured'];
    allowSymlinks = json['allow_symlinks'];
    roles = List.castFrom<dynamic, String>(json['roles']);
    passphrase = json['passphrase'];
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
