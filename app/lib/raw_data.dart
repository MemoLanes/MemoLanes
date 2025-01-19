import 'package:flutter/material.dart';
import 'package:memolanes/src/rust/api/api.dart';
import 'package:memolanes/src/rust/storage.dart';
import 'package:memolanes/utils.dart';
import 'package:share_plus/share_plus.dart';

class RawDataSwitch extends StatefulWidget {
  const RawDataSwitch({super.key});

  @override
  State<RawDataSwitch> createState() => _RawDataSwitchState();
}

class _RawDataSwitchState extends State<RawDataSwitch> {
  bool enabled = false;

  @override
  initState() {
    super.initState();
    getRawDataMode().then((value) => setState(() {
          enabled = value;
        }));
  }

  @override
  Widget build(BuildContext context) {
    return Switch(
      value: enabled,
      activeColor: Colors.red,
      onChanged: (bool value) async {
        await toggleRawDataMode(enable: value);
        setState(() {
          enabled = value;
        });
      },
    );
  }
}

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

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          const Text("Raw Data Mode",
              style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold)),
          const SizedBox(height: 8),
          const RawDataSwitch(),
          const SizedBox(height: 16),
          Expanded(
            child: ListView(
              shrinkWrap: true,
              children: items.map((item) {
                return ListTile(
                  leading: const Icon(Icons.description),
                  title: Text(item.name),
                  onTap: () {
                    Share.shareXFiles([XFile(item.path)]);
                  },
                  trailing: ElevatedButton(
                    onPressed: () async {
                      if (await showCommonDialog(context, "Delete this record?",
                          showCancel: true,
                          title: "Delete",
                          confirmGroundColor: Colors.red,
                          confirmTextColor: Colors.white)) {
                        await deleteRawDataFile(filename: item.name);
                        _loadList();
                      }
                    },
                    child: const Icon(Icons.delete),
                  ),
                );
              }).toList(),
            ),
          ),
        ],
      ),
    );
  }
}
