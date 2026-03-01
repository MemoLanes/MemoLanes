import 'package:flutter/material.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/common/utils.dart';

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
  DateTime? _earliestJourneyDate;
  bool _loading = false;
  DateTime? _lastFrom;
  DateTime? _lastTo;

  Future<void> _loadJourneyForRange(DateTime from, DateTime to) async {
    if (_earliestJourneyDate == null) return;
    if (from.isAfter(to)) return;
    _lastFrom = from;
    _lastTo = to;
    setState(() => _loading = true);
    try {
      final proxy = await api.getMapRendererProxyForJourneyDateRange(
        fromDateInclusive: dateTimeToNaiveDate(from),
        toDateInclusive: dateTimeToNaiveDate(to),
      );
      if (mounted) widget.onJourneyRangeLoaded(proxy);
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _onLayerFilterChanged() {
    final from = _lastFrom;
    final to = _lastTo;
    if (from != null && to != null) {
      _loadJourneyForRange(from, to);
    }
  }

  @override
  void initState() {
    super.initState();
    api.earliestJourneyDate().then((value) {
      if (!mounted) return;
      setState(() {
        _earliestJourneyDate = value != null
            ? naiveDateToDateTime(value)
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

    final isLandscape =
        MediaQuery.of(context).orientation == Orientation.landscape;

    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 24),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Spacer(),
            Padding(
              padding: EdgeInsets.only(
                bottom: isLandscape ? 16 : 8,
              ),
              child: TimeRangePicker(
                earliestDate: earliest,
                loading: _loading,
                onRangeChanged: _loadJourneyForRange,
                onLayerFilterChanged: _onLayerFilterChanged,
              ),
            ),
            const SizedBox(height: 116),
          ],
        ),
      ),
    );
  }
}
