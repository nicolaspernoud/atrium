import 'dart:math';

import 'package:flutter/material.dart';

class SizedItemsGrid extends StatefulWidget {
  final Widget Function(BuildContext, int) itemBuilder;
  final List<dynamic> list;

  const SizedItemsGrid(
      {super.key, required this.itemBuilder, required this.list});

  @override
  State<SizedItemsGrid> createState() => _SizedItemsGridState();
}

class _SizedItemsGridState extends State<SizedItemsGrid> {
  static const tileSize = 195;

  @override
  Widget build(BuildContext context) {
    var width = MediaQuery.of(context).size.width;
    var crossAxisCount = min(widget.list.length, (width / tileSize)).floor();
    var padding = (width - crossAxisCount * tileSize) / 2;

    if (widget.list.isEmpty) {
      return Container();
    }

    return GridView.builder(
      padding: EdgeInsets.symmetric(vertical: 12.0, horizontal: padding),
      itemCount: widget.list.length,
      itemBuilder: widget.itemBuilder,
      gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: crossAxisCount,
      ),
    );
  }
}
