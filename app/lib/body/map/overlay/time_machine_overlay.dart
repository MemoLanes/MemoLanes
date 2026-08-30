import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';

/// Initial layer selection for time machine: ensure at least default kind (from main map filter).
Set<JourneyKind> _initialJourneyKindsFromMainMap() {
  final f = api.getCurrentMainMapLayerFilter();
  final defaultOn = f.defaultKind;
  final flightOn = f.flightKind;
  if (!defaultOn && !flightOn) return {JourneyKind.defaultKind};
  if (defaultOn && flightOn) {
    return {JourneyKind.defaultKind, JourneyKind.flight};
  }
  if (defaultOn) return {JourneyKind.defaultKind};
  return {JourneyKind.flight};
}

class TimeMachineOverlay extends StatefulWidget {
  const TimeMachineOverlay({super.key, required this.onJourneyRangeLoaded});

  final void Function(api.MapRendererProxy? proxy) onJourneyRangeLoaded;

  @override
  State<TimeMachineOverlay> createState() => _TimeMachineOverlayState();
}

class _TimeMachineOverlayState extends State<TimeMachineOverlay> {
  SimpleDate? _earliestJourneyDate;
  bool _loading = false;
  SimpleDate? _lastFrom;
  SimpleDate? _lastTo;

  late Set<JourneyKind> _selectedJourneyKinds;

  @override
  void initState() {
    super.initState();
    _selectedJourneyKinds = _initialJourneyKindsFromMainMap();
    api.earliestJourneyDate().then((value) {
      if (!mounted) return;
      setState(() {
        _earliestJourneyDate =
            value?.toSimpleDate() ?? SimpleDate(DateTime.now().year);
      });
    });
  }

  Future<void> _loadJourneyForRange(SimpleDate from, SimpleDate to) async {
    if (_earliestJourneyDate == null) return;
    if (from.isAfter(to)) return;
    _lastFrom = from;
    _lastTo = to;
    setState(() => _loading = true);
    try {
      final proxy = await api.getMapRendererProxyForJourneyDateRange(
        fromDateInclusive: from.toFrbNaiveDate(),
        toDateInclusive: to.toFrbNaiveDate(),
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

    final padding = MediaQuery.viewPaddingOf(context);
    final horizontalSafeArea = math.max(padding.left, padding.right);

    return Stack(
      children: [
        Positioned(
          left: horizontalSafeArea + 24,
          right: horizontalSafeArea + 24,
          bottom: StyleConstants.mapPrimaryControlBottomInsetForContext(
            context,
          ),
          child: TimeRangePicker(
            earliestDate: earliest,
            loading: _loading,
            onRangeChanged: _loadJourneyForRange,
            selectedJourneyKinds: _selectedJourneyKinds,
            onJourneyKindsChanged: _onJourneyKindsChanged,
          ),
        ),
      ],
    );
  }
}
