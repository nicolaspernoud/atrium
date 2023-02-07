import 'package:flutter/widgets.dart';

abstract class Model {
  late String host;
  late Color color;
  late String icon;
}

class AppModel implements Model {
  AppModel({
    required this.id,
    this.name = "",
    this.icon = "web_asset",
    this.color = const Color(0xffd32f2f),
    this.isProxy = true,
    this.host = "",
    this.subdomains = const [],
    this.target = "",
    this.secured = true,
    this.login = "",
    this.password = "",
    this.openpath = "",
    this.roles = const ["ADMINS", "USERS"],
    this.injectSecurityHeaders = true,
    this.forwardUserMail = false,
  });

  late int id;
  late String name;
  @override
  late String icon;
  @override
  late Color color;
  late bool isProxy;
  @override
  late String host;
  late List<String> subdomains;
  late String target;
  late bool secured;
  late String login;
  late String password;
  late String openpath;
  late List<String> roles;
  late bool injectSecurityHeaders;
  late bool forwardUserMail;

  AppModel.fromJson(Map<String, dynamic> json) {
    id = json['id'];
    name = json['name'];
    icon = json['icon'] != "" ? json['icon'] : "web_asset";
    color = Color(json['color']);
    isProxy = json['is_proxy'] ?? false;
    host = json['host'];
    subdomains = json['subdomains'] != null
        ? List.castFrom<dynamic, String>(json['subdomains'])
        : [];
    target = json['target'];
    secured = json['secured'] ?? false;
    login = json['login'] ?? "";
    password = json['password'] ?? "";
    openpath = json['openpath'] ?? "";
    roles = json['roles'] != null
        ? List.castFrom<dynamic, String>(json['roles'])
        : [];
    injectSecurityHeaders = json['inject_security_headers'] ?? false;
    forwardUserMail = json['forward_user_mail'] ?? false;
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['id'] = id;
    data['name'] = name;
    data['icon'] = icon;
    data['color'] = color.value;
    data['is_proxy'] = isProxy;
    data['host'] = host;
    data['subdomains'] =
        subdomains.isNotEmpty && subdomains[0].isNotEmpty ? subdomains : null;
    data['target'] = target;
    data['secured'] = secured;
    data['login'] = login;
    data['password'] = password;
    data['openpath'] = openpath;
    data['roles'] = roles;
    data['inject_security_headers'] = injectSecurityHeaders;
    data['forward_user_mail'] = forwardUserMail;
    return data;
  }
}
