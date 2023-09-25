class UserModel {
  UserModel({
    this.login = "",
    this.password = "",
    this.roles = const ["ADMINS", "USERS"],
    this.givenName = "",
    this.familyName = "",
    this.email = "",
  });

  late String login;
  late String password;
  late List<String> roles;
  late String givenName;
  late String familyName;
  late String email;
  bool isDeleting = false;

  UserModel.fromJson(Map<String, dynamic> json) {
    login = json['login'];
    password = json['password'];
    roles = List.castFrom<dynamic, String>(json['roles']);
    givenName = json['info']?['given_name'] ?? "";
    familyName = json['info']?['family_name'] ?? "";
    email = json['info']?['email'] ?? "";
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['login'] = login;
    data['password'] = password;
    data['roles'] = roles;
    if (givenName != "" || familyName != "" || email != "") {
      data['info'] = <String, dynamic>{};
      data['info']['given_name'] = givenName;
      data['info']['family_name'] = familyName;
      data['info']['email'] = email;
    }
    return data;
  }
}
