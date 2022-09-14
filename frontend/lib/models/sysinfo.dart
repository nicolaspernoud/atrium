import 'package:filesize/filesize.dart';

class SysInfo {
  SysInfo({
    required this.totalMemory,
    required this.usedMemory,
    required this.cpuUsagePercent,
    required this.uptime,
  });
  late final int totalMemory;
  late final int usedMemory;
  late final double cpuUsagePercent;
  late final int uptime;

  double get memoryUsage {
    return usedMemory / totalMemory;
  }

  double get cpuUsage {
    return cpuUsagePercent / 100;
  }

  String get usedMemoryLabel {
    return '${filesize(usedMemory)} / ${filesize(totalMemory)} - ${memoryUsage * 100} %';
  }

  SysInfo.fromJson(Map<String, dynamic> json) {
    totalMemory = json['total_memory'];
    usedMemory = json['used_memory'];
    cpuUsagePercent = json['cpu_usage'];
    uptime = json['uptime'];
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['total_memory'] = totalMemory;
    data['used_memory'] = usedMemory;
    data['cpu_usage'] = cpuUsagePercent;
    data['uptime'] = uptime;
    return data;
  }
}

class DiskInfo {
  DiskInfo({
    required this.name,
    required this.mountPoint,
    required this.availableSpace,
    required this.totalSpace,
  });
  late final String name;
  late final String mountPoint;
  late final int availableSpace;
  late final int totalSpace;

  int get usedSpace {
    return totalSpace - availableSpace;
  }

  double get spaceUsage {
    return usedSpace / totalSpace;
  }

  String get usedSpaceLabel {
    return '${filesize(usedSpace)} / ${filesize(totalSpace)}';
  }

  DiskInfo.fromJson(Map<String, dynamic> json) {
    name = json['name'];
    mountPoint = json['mount_point'];
    availableSpace = json['available_space'];
    totalSpace = json['total_space'];
  }

  Map<String, dynamic> toJson() {
    final data = <String, dynamic>{};
    data['name'] = name;
    data['mount_point'] = mountPoint;
    data['available_space'] = availableSpace;
    data['total_space'] = totalSpace;
    return data;
  }
}
