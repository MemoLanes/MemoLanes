import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';

class TimeMachineUIBody extends StatefulWidget {
  const TimeMachineUIBody({super.key});

  @override
  State<TimeMachineUIBody> createState() => _TimeMachineUIBodyState();
}

class _TimeMachineUIBodyState extends State<TimeMachineUIBody> {
  final DateFormat dateFormat = DateFormat("yyyy-MM-dd");
  DateTime? earliestJourneyDate;

  DateTime fromDateInclusive = DateTime.now();
  DateTime toDateInclusive = DateTime.now();

  bool loading = false;

  @override
  void initState() {
    super.initState();

    api.earliestJourneyDate().then((value) {
      if (value != null) {
        setState(() {
          earliestJourneyDate =
              dateFormat.parse(naiveDateToString(date: value));
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    var earliestJourneyDate = this.earliestJourneyDate;
    if (earliestJourneyDate == null) {
      return const Center(
          child: Text('No Data', style: TextStyle(fontSize: 24)));
    }
    return Center(
      child: Column(
        mainAxisAlignment: MainAxisAlignment.start,
        children: <Widget>[
          Container(
              padding: const EdgeInsets.all(10),
              child: const Text("Naive TimeMachine",
                  style: TextStyle(fontSize: 20))),
          TextField(
            readOnly: true,
            controller: TextEditingController(
                text: dateFormat.format(fromDateInclusive)),
            onTap: () async {
              DateTime? time = await showDatePicker(
                context: context,
                initialDate: fromDateInclusive,
                firstDate: earliestJourneyDate,
                lastDate: DateTime.now(),
              );
              if (time != null) {
                setState(() {
                  fromDateInclusive = time;
                });
              }
            },
            decoration: const InputDecoration(
              label: Text("From: "),
            ),
          ),
          TextField(
            readOnly: true,
            controller:
                TextEditingController(text: dateFormat.format(toDateInclusive)),
            onTap: () async {
              DateTime? time = await showDatePicker(
                context: context,
                initialDate: toDateInclusive,
                firstDate: earliestJourneyDate,
                lastDate: DateTime.now(),
              );
              if (time != null) {
                setState(() {
                  toDateInclusive = time;
                });
              }
            },
            decoration: const InputDecoration(
              label: Text("To: "),
            ),
          ),
          Container(
              padding: const EdgeInsets.all(10),
              child: ElevatedButton(
                  onPressed: (loading
                      ? null
                      : () async {
                          setState(() {
                            loading = true;
                          });
                          setState(() {
                            loading = false;
                          });
                        }),
                  child: const Text("View"))),
          const Expanded(
            child: Text("aaaa"),
          ),
        ],
      ),
    );
  }
}
