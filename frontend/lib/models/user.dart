class UserModel {
  UserModel({
    this.login = "",
    this.password = "",
    this.roles = const ["ADMINS", "USERS"],
    this.firstname = "",
    this.lastname = "",
    this.email = "",
  });

  late String login;
  late String password;
  late List<String> roles;
  late String firstname;
  late String lastname;
  late String email;

  UserModel.fromJson(Map<String, dynamic> json) {
    login = json['login'];
    password = json['password'];
    roles = List.castFrom<dynamic, String>(json['roles']);
    firstname = json['info']?['firstname'] ?? "";
    lastname = json['info']?['lastname'] ?? "";
    email = json['info']?['email'] ?? "";
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['login'] = login;
    data['password'] = password;
    data['roles'] = roles;
    data['info'] = <String, dynamic>{};
    data['info']['firstname'] = firstname;
    data['info']['lastname'] = lastname;
    data['info']['email'] = email;
    return data;
  }
}
