class PathItem {
  final PathType pathType;
  final String name;
  final int? mtime;
  final int? size;

  PathItem({
    required this.pathType,
    required this.name,
    required this.mtime,
    this.size,
  });

  PathItem.fromJson(Map<String, dynamic> json)
      : pathType = json['path_type'] == "Dir" ? PathType.dir : PathType.file,
        name = json['name'] as String,
        mtime = json['mtime'] as int?,
        size = json['size'] as int?;

  Map<String, dynamic> toJson() =>
      {'path_type': pathType, 'name': name, 'mtime': mtime, 'size': size};
}

enum PathType { file, dir }
