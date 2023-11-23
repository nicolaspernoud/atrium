import 'dart:math';
import 'package:flutter/material.dart';

Color colorFromPercent(double? percent) {
  if (percent == null) return Colors.grey;
  if (percent > 0.80) return Colors.red;
  if (percent > 0.70) return Colors.orange;
  if (percent > 0.60) return Colors.yellow;
  return Colors.green;
}

String generateRandomString(int length) {
  final random = Random();
  const availableChars =
      'AaBbCcDdEeFfGgHhIiJjKkLlMmNnOoPpQqRrSsTtUuVvWwXxYyZz1234567890';
  return List.generate(length,
      (index) => availableChars[random.nextInt(availableChars.length)]).join();
}

enum FileType { text, document, image, media, pdf, other }

FileType fileTypeFromExt(String ext) {
  if (ext == "pdf") return FileType.pdf;
  if ([
    "csv",
    "json",
    "log",
    "md",
    "nfo",
    "py",
    "sh",
    "srt",
    "txt",
    "yaml",
    "yml",
  ].contains(ext)) return FileType.text;
  if (["docx", "doc", "odt", "xlsx", "xls", "ods", "pptx", "ppt", "opd"]
      .contains(ext)) return FileType.document;
  if ([
    "apng",
    "avif",
    "bmp",
    "cur",
    "gif",
    "ico",
    "jfif",
    "jpeg",
    "jpg",
    "pjp",
    "pjpeg",
    "png",
    "svg",
    "tif",
    "tiff",
    "webp"
  ].contains(ext)) return FileType.image;
  if (["mp3", "wav", "ogg", "mp4", "avi", "mkv", "m4v", "webm"].contains(ext)) {
    return FileType.media;
  }
  return FileType.other;
}
