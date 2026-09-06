import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/async_load_token.dart';
import 'package:memolanes/common/log.dart';
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
  static const _rangeLoadDebounce = Duration(milliseconds: 100);
  static const _loadErrorMessage =
      '[TimeMachineOverlay] failed to load journey range';

  SimpleDate? _earliestJourneyDate;
  bool _loading = false;
  SimpleDate? _requestedFrom;
  SimpleDate? _requestedTo;
  final _loadGuard = AsyncLoadToken();
  Timer? _rangeLoadDebounceTimer;

  late Set<JourneyKind> _selectedJourneyKinds;

  @override
  void initState() {
    super.initState();
    _selectedJourneyKinds = _initialJourneyKindsFromMainMap();
    unawaited(_loadEarliestJourneyDate());
  }

  Future<void> _loadEarliestJourneyDate() async {
    try {
      final value = await api.earliestJourneyDate();
      if (!mounted) return;
      setState(() {
        _earliestJourneyDate =
            value?.toSimpleDate() ?? SimpleDate(DateTime.now().year);
      });
    } catch (error, stackTrace) {
      log.error(
        '[TimeMachineOverlay] failed to load earliest journey date: $error',
        stackTrace,
      );
    }
  }

  Future<void> _loadJourneyForRange(SimpleDate from, SimpleDate to) async {
    _rangeLoadDebounceTimer?.cancel();
    _rangeLoadDebounceTimer = null;
    if (!mounted) return;
    if (_earliestJourneyDate == null) return;
    if (from.isAfter(to)) return;
    final loadToken = _loadGuard.begin();
    _requestedFrom = from;
    _requestedTo = to;
    final journeyKinds = Set<JourneyKind>.from(_selectedJourneyKinds);
    setState(() => _loading = true);
    try {
      final proxy = await api.getMapRendererProxyForJourneyDateRange(
        fromDateInclusive: from.toFrbNaiveDate(),
        toDateInclusive: to.toFrbNaiveDate(),
        journeyKinds: journeyKinds,
      );
      if (mounted && _loadGuard.isActive(loadToken)) {
        widget.onJourneyRangeLoaded(proxy);
      }
    } catch (error, stackTrace) {
      log.error('$_loadErrorMessage: $error', stackTrace);
    } finally {
      if (mounted && _loadGuard.isActive(loadToken)) {
        setState(() => _loading = false);
        _loadGuard.clear();
      }
    }
  }

  void _scheduleJourneyRangeLoad(SimpleDate from, SimpleDate to) {
    _requestedFrom = from;
    _requestedTo = to;
    _rangeLoadDebounceTimer?.cancel();
    _rangeLoadDebounceTimer = Timer(_rangeLoadDebounce, () {
      _rangeLoadDebounceTimer = null;
      unawaited(_loadJourneyForRange(from, to));
    });
  }

  void _onJourneyKindsChanged(Set<JourneyKind> newKinds) {
    final kinds = Set<JourneyKind>.from(newKinds);
    setState(() => _selectedJourneyKinds = kinds);
    final from = _requestedFrom;
    final to = _requestedTo;
    if (from != null && to != null) {
      unawaited(_loadJourneyForRange(from, to));
    }
  }

  @override
  void dispose() {
    _rangeLoadDebounceTimer?.cancel();
    _loadGuard.clear();
    super.dispose();
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
            onRangeChanged: _scheduleJourneyRangeLoad,
            onRangeCommitted: _loadJourneyForRange,
            selectedJourneyKinds: _selectedJourneyKinds,
            onJourneyKindsChanged: _onJourneyKindsChanged,
          ),
        ),
      ],
    );
  }
}
