import 'package:flutter/material.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/common/utils.dart';

/// Initial layer selection for time machine: ensure at least default kind (from main map filter).
Set<JourneyKind> _initialJourneyKindsFromMainMap() {
  final f = api.getCurrentMainMapLayerFilter();
  final defaultOn = f.defaultKind;
  final flightOn = f.flightKind;
  if (!defaultOn && !flightOn) return {JourneyKind.defaultKind};
  if (defaultOn && flightOn) return {JourneyKind.defaultKind, JourneyKind.flight};
  if (defaultOn) return {JourneyKind.defaultKind};
  return {JourneyKind.flight};
}

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

  late Set<JourneyKind> _selectedJourneyKinds;

  @override
  void initState() {
    super.initState();
    _selectedJourneyKinds = _initialJourneyKindsFromMainMap();
    api.earliestJourneyDate().then((value) {
      if (!mounted) return;
      setState(() {
        _earliestJourneyDate = value != null
            ? naiveDateToDateTime(value)
            : DateTime(DateTime.now().year, 1, 1);
      });
    });
  }

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
        journeyKinds: _selectedJourneyKinds,
      );
      if (mounted) widget.onJourneyRangeLoaded(proxy);
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _onJourneyKindsChanged(Set<JourneyKind> newKinds) {
    setState(() => _selectedJourneyKinds = newKinds);
    final from = _lastFrom;
    final to = _lastTo;
    if (from != null && to != null) {
      _loadJourneyForRange(from, to);
    }
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
                selectedJourneyKinds: _selectedJourneyKinds,
                onJourneyKindsChanged: _onJourneyKindsChanged,
              ),
            ),
            const SizedBox(height: 116),
          ],
        ),
      ),
    );
  }
}
