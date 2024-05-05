import 'package:flutter/material.dart';
import 'package:project_dv/src/rust/api/api.dart';
import 'package:project_dv/src/rust/storage.dart';
import 'package:share_plus/share_plus.dart';

class RawDataBody extends StatefulWidget {
  const RawDataBody({super.key});

  @override
  State<RawDataBody> createState() => _RawDataBody();
}

class _RawDataBody extends State<RawDataBody> {
  List<RawDataFile> items = [];

  @override
  void initState() {
    super.initState();
    _loadList();
  }

  _loadList() async {
    var list = await listAllRawData();
    setState(() {
      items = list;
    });
  }

  showDialogFunction(fn) {
    showDialog(
      context: context,
      builder: (BuildContext context) {
        return AlertDialog(
          title: const Text("Delete"),
          content: const Text("Delete this record?"),
          actions: [
            TextButton(
              onPressed: () {
                Navigator.of(context).pop();
              },
              child: const Text('Cancel'),
            ),
            TextButton(onPressed: fn, child: const Text("Yes")),
          ],
        );
      },
    );
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      shrinkWrap: false,
      //沿竖直方向上布局
      scrollDirection: Axis.vertical,
      padding: const EdgeInsets.fromLTRB(0, 30, 0, 30),
      children: items.map((item) {
        return ListTile(
            leading: const Icon(Icons.description),
            title: Text(item.name),
            onTap: () {
              Share.shareXFiles([XFile(item.path)]);
            },
            trailing: ElevatedButton(
              onPressed: () async {
                showDialogFunction(() async {
                  await deleteRawDataFile(filename: item.name);
                  _loadList();
                  Navigator.of(context).pop();
                });
              },
              child: const Icon(Icons.delete),
            ));
      }).toList(),
    );
  }
}
