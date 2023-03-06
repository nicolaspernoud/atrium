import 'package:flutter/material.dart';

import '../i18n.dart';
import 'icons.dart';

class IconPicker extends StatefulWidget {
  final String? currentValue;
  const IconPicker({super.key, this.currentValue});

  @override
  State<IconPicker> createState() => _IconPickerState();
}

class _IconPickerState extends State<IconPicker> {
  String _filter = "";

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(tr(context, 'pick_an_icon')),
      content: SizedBox(
          height: MediaQuery.of(context).size.height * 0.8,
          width: MediaQuery.of(context).size.width * 0.8,
          child: GridView.extent(
            maxCrossAxisExtent: 100,
            children: roundedIcons.entries
                .where((element) => element.key.contains(_filter))
                .map((i) => Container(
                      decoration: i.key == widget.currentValue
                          ? BoxDecoration(
                              border:
                                  Border.all(color: Colors.indigo, width: 2),
                              shape: BoxShape.circle,
                            )
                          : null,
                      child: IconButton(
                          icon: Icon(
                            i.value,
                            size: 50,
                          ),
                          onPressed: () {
                            Navigator.pop(context, i.key);
                          }),
                    ))
                .toList(),
          )),
      actions: <Widget>[
        Padding(
          padding: const EdgeInsets.only(left: 16, right: 16, bottom: 16),
          child: TextFormField(
              initialValue: _filter,
              decoration: InputDecoration(
                  icon: const Icon(Icons.search),
                  labelText: tr(context, 'search')),
              onChanged: (value) {
                setState(() {
                  _filter = value;
                });
              }),
        )
      ],
    );
  }
}
