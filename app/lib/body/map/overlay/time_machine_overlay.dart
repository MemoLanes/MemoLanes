import 'package:flutter/material.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
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
  static final DateFormat _dateFormat = DateFormat('yyyy-MM-dd');
  DateTime? _earliestJourneyDate;
  bool _loading = false;

  Future<void> _loadJourneyForRange(DateTime from, DateTime to) async {
    if (_earliestJourneyDate == null) return;
    if (from.isAfter(to)) return;
    setState(() => _loading = true);
    try {
      final proxy = await api.getMapRendererProxyForJourneyDateRange(
        fromDateInclusive: naiveDateOfString(str: _dateFormat.format(from)),
        toDateInclusive: naiveDateOfString(str: _dateFormat.format(to)),
      );
      if (mounted) widget.onJourneyRangeLoaded(proxy);
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  @override
  void initState() {
    super.initState();
    api.earliestJourneyDate().then((value) {
      if (!mounted) return;
      setState(() {
        _earliestJourneyDate = value != null
            ? _dateFormat.parse(naiveDateToString(date: value))
            : DateTime(DateTime.now().year, 1, 1);
      });
    });
  }

  @override
  Widget build(BuildContext context) {
    final earliest = _earliestJourneyDate;
    if (earliest == null) {
      return const SizedBox.shrink();
    }

    final screenSize = MediaQuery.of(context).size;
    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.center,
          children: [
            const Spacer(),
            Padding(
              padding: EdgeInsets.only(
                bottom: isLandscape ? 40 : screenSize.height * 0.12,
              ),
              child: TimeRangePicker(
                earliestDate: earliest,
                loading: _loading,
                onRangeChanged: _loadJourneyForRange,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
