import 'package:flutter/widgets.dart';

abstract class Model {
  late String host;
  late Color color;
  late int icon;
}

class AppModel implements Model {
  AppModel({
    required this.id,
    this.name = "",
    this.icon = 63298,
    this.color = const Color(0xffd32f2f),
    this.isProxy = true,
    this.host = "",
    this.target = "",
    this.secured = true,
    this.login = "",
    this.password = "",
    this.openpath = "",
    this.roles = const ["ADMINS", "USERS"],
    this.injectSecurityHeaders = true,
  });

  late int id;
  late String name;
  @override
  late int icon;
  @override
  late Color color;
  late bool isProxy;
  @override
  late String host;
  late String target;
  late bool secured;
  late String login;
  late String password;
  late String openpath;
  late List<String> roles;
  late bool injectSecurityHeaders;

  AppModel.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    name = json['name'];
    icon = json['icon'];
    color = Color(json['color']);
    isProxy = json['is_proxy'];
    host = json['host'];
    target = json['target'];
    secured = json['secured'];
    login = json['login'];
    password = json['password'];
    openpath = json['openpath'];
    roles = List.castFrom<dynamic, String>(json['roles']);
    injectSecurityHeaders = json['inject_security_headers'];
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['id'] = id;
    data['name'] = name;
    data['icon'] = icon;
    data['color'] = color.value;
    data['is_proxy'] = isProxy;
    data['host'] = host;
    data['target'] = target;
    data['secured'] = secured;
    data['login'] = login;
    data['password'] = password;
    data['openpath'] = openpath;
    data['roles'] = roles;
    data['inject_security_headers'] = injectSecurityHeaders;
    return data;
  }
}
