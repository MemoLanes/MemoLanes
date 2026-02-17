import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';

class TimeMachineOverlay extends StatefulWidget {
  const TimeMachineOverlay({
    super.key,
    required this.onJourneyRangeLoaded,
  });

  final void Function(api.MapRendererProxy? proxy) onJourneyRangeLoaded;

  @override
  State<TimeMachineOverlay> createState() => _TimeMachineOverlayState();
}

class _TimeMachineOverlayState extends State<TimeMachineOverlay> {
  final DateFormat _dateFormat = DateFormat("yyyy-MM-dd");
  DateTime? _earliestJourneyDate;

  DateTime _fromDateInclusive = DateTime.now();
  DateTime _toDateInclusive = DateTime.now();

  bool _loading = false;
  bool _changed = true;

  @override
  void initState() {
    super.initState();
    api.earliestJourneyDate().then((value) {
      if (value != null && mounted) {
        setState(() {
          _earliestJourneyDate =
              _dateFormat.parse(naiveDateToString(date: value));
        });
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final earliestJourneyDate = _earliestJourneyDate;
    if (earliestJourneyDate == null) {
      return const Center(
        child: Text('No Data', style: TextStyle(fontSize: 24)),
      );
    }

    return SafeArea(
      child: Center(
        child: Column(
          mainAxisAlignment: MainAxisAlignment.start,
          children: <Widget>[
            Container(
              padding: const EdgeInsets.all(10),
              child: const Text(
                "Naive TimeMachine",
                style: TextStyle(fontSize: 20),
              ),
            ),
            TextField(
              readOnly: true,
              controller: TextEditingController(
                text: _dateFormat.format(_fromDateInclusive),
              ),
              onTap: () async {
                final time = await showDatePicker(
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
                text: _dateFormat.format(_toDateInclusive),
              ),
              onTap: () async {
                final time = await showDatePicker(
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
                onPressed: (_loading || !_changed)
                    ? null
                    : () async {
                        setState(() {
                          _loading = true;
                          _changed = false;
                        });
                        try {
                          final mapRendererProxy =
                              await api.getMapRendererProxyForJourneyDateRange(
                            fromDateInclusive: naiveDateOfString(
                              str: _dateFormat.format(_fromDateInclusive),
                            ),
                            toDateInclusive: naiveDateOfString(
                              str: _dateFormat.format(_toDateInclusive),
                            ),
                          );
                          if (mounted) {
                            widget.onJourneyRangeLoaded(mapRendererProxy);
                          }
                        } finally {
                          if (mounted) {
                            setState(() => _loading = false);
                          }
                        }
                      },
                child: Text(_loading ? "Loading" : "View"),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
