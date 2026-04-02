class ShareResponseModel {
  ShareResponseModel({this.token = "", this.xsrfToken = ""});

  late String token;
  late String xsrfToken;

  ShareResponseModel.fromJson(Map<String, dynamic> json) {
    token = json['token'];
    xsrfToken = json['xsrf_token'];
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['token'] = token;
    data['xsrf_token'] = xsrfToken;
    return data;
  }
}
