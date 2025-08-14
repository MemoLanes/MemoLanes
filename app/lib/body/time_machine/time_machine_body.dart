import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/common/component/base_map_webview.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';

class TimeMachineBody extends StatefulWidget {
  const TimeMachineBody({super.key});

  @override
  State<TimeMachineBody> createState() => _TimeMachineBodyState();
}

class _TimeMachineBodyState extends State<TimeMachineBody> {
  final DateFormat _dateFormat = DateFormat("yyyy-MM-dd");
  DateTime? _earliestJourneyDate;

  DateTime _fromDateInclusive = DateTime.now();
  DateTime _toDateInclusive = DateTime.now();

  bool _loading = false;
  bool _changed = true;

  api.MapRendererProxy? _mapRendererProxy;

  @override
  void initState() {
    super.initState();

    api.earliestJourneyDate().then((value) {
      if (value != null) {
        setState(() {
          _earliestJourneyDate =
              _dateFormat.parse(naiveDateToString(date: value));
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    var earliestJourneyDate = _earliestJourneyDate;
    if (earliestJourneyDate == null) {
      return const Center(
          child: Text('No Data', style: TextStyle(fontSize: 24)));
    }

    var mapRendererProxy = _mapRendererProxy;
    var mapComponent = (mapRendererProxy == null)
        ? Container()
        : BaseMapWebview(
            key: const ValueKey("mapWidget"),
            mapRendererProxy: mapRendererProxy,
            // TODO: get a reasonable camera option from the journey bitmap.
          );

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
                text: _dateFormat.format(_fromDateInclusive)),
            onTap: () async {
              DateTime? time = await showDatePicker(
                context: context,
                initialDate: _fromDateInclusive,
                firstDate: earliestJourneyDate,
                lastDate: DateTime.now(),
              );
              if (time != null) {
                setState(() {
                  _changed = true;
                  _fromDateInclusive = time;
                });
              }
            },
            decoration: const InputDecoration(
              label: Text("From: "),
            ),
          ),
          TextField(
            readOnly: true,
            controller: TextEditingController(
                text: _dateFormat.format(_toDateInclusive)),
            onTap: () async {
              DateTime? time = await showDatePicker(
                context: context,
                initialDate: _toDateInclusive,
                firstDate: earliestJourneyDate,
                lastDate: DateTime.now(),
              );
              if (time != null) {
                setState(() {
                  _changed = true;
                  _toDateInclusive = time;
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
                  onPressed: ((_loading || !_changed)
                      ? null
                      : () async {
                          setState(() {
                            _loading = true;
                            _changed = false;
                          });
                          var mapRendererProxy =
                              await api.getMapRendererProxyForJourneyDateRange(
                                  fromDateInclusive: naiveDateOfString(
                                      str: _dateFormat
                                          .format(_fromDateInclusive)),
                                  toDateInclusive: naiveDateOfString(
                                      str: _dateFormat
                                          .format(_toDateInclusive)));
                          setState(() {
                            _mapRendererProxy = mapRendererProxy;
                            _loading = false;
                          });
                        }),
                  child: Text(_loading ? "Loading" : "View"))),
          Expanded(
            child: mapComponent,
          ),
        ],
      ),
    );
  }
}
