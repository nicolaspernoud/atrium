class ExploreConfig {
  String dav;
  String path;
  bool writable;

  ExploreConfig({
    required this.dav,
    required this.path,
    required this.writable,
  });

  factory ExploreConfig.fromJson(Map<String, dynamic> json) {
    return ExploreConfig(
      dav: json['dav'],
      path: json['path'],
      writable: json['writable'],
    );
  }

  Map<String, dynamic> toJson() {
    return {
      'dav': dav,
      'path': path,
      'writable': writable,
    };
  }
}
