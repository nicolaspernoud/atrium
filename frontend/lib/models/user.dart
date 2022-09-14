class UserModel {
  UserModel({
    this.login = "",
    this.password = "",
    this.roles = const ["ADMINS", "USERS"],
  });

  late String login;
  late String password;
  late List<String> roles;

  UserModel.fromJson(Map<String, dynamic> json) {
    login = json['login'];
    password = json['password'];
    roles = List.castFrom<dynamic, String>(json['roles']);
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['login'] = login;
    data['password'] = password;
    data['roles'] = roles;
    return data;
  }
}
