import 'package:atrium/utils.dart';
import 'package:dio/dio.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:webdav_client/webdav_client.dart';

FileType fileType(File? file) {
  if (file == null || file.name == null) return FileType.other;
  var ext = file.name!.split(".").last.toLowerCase();
  return fileTypeFromExt(ext);
}

// Sort : folders first and then alphabetically
int foldersFirstThenAlphabetically(File a, File b) {
  if (a.isDir! && !(b.isDir!)) {
    return -1;
  }
  if (!(a.isDir!) && (b.isDir!)) {
    return 1;
  }
  if (a.name != null && b.name != null) {
    return a.name!.compareTo(b.name!);
  }
  return 0;
}

String formatTime(DateTime? d) {
  if (d == null) return "-";
  return "${d.year.toString()}-${d.month.toString().padLeft(2, "0")}-${d.day.toString().padLeft(2, "0")} ${d.hour.toString().padLeft(2, "0")}:${d.minute.toString().padLeft(2, "0")}:${d.second.toString().padLeft(2, "0")}";
}

String? joinQueryParameters(Map<String, String> params) {
  return params.entries
      .map((MapEntry<String, String> e) => '${e.key}=${e.value}')
      .join('&');
}

/// Leading widget for a file tile: an icon for its [type], or a
/// thumbnail for images.
Widget fileTypeWidget(
  Client client,
  File file,
  FileType type,
  CancelToken cancelToken,
) {
  if (file.isDir != null && file.isDir!) {
    return const Icon(Icons.folder, size: 30);
  }
  switch (type) {
    case FileType.document:
      return const Icon(Icons.article, size: 30);
    case FileType.image:
      return SizedBox(
        width: 30,
        height: 30,
        child: FutureBuilder<ImageProvider>(
          future: client
              .read(file.path!, cancelToken: cancelToken)
              .then((value) => MemoryImage(Uint8List.fromList(value))),
          builder:
              (BuildContext context, AsyncSnapshot<ImageProvider> snapshot) {
                if (snapshot.hasData) {
                  return Center(child: Image(image: snapshot.data!));
                } else {
                  return const Icon(Icons.image, size: 30);
                }
              },
        ),
      );
    case FileType.media:
      return const Icon(Icons.play_circle, size: 30);
    case FileType.pdf:
      return const Icon(Icons.picture_as_pdf, size: 30);
    case FileType.text:
      return const Icon(Icons.description, size: 30);
    default:
      return const Icon(Icons.file_present_rounded, size: 30);
  }
}
