import 'dart:async';
import 'dart:ui';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:intl/intl.dart';
import 'package:memolanes/body/time_machine/time_range_picker.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/api/utils.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

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
        fromDateInclusive: naiveDateOfString(str: _dateFormat.format(from)),
        toDateInclusive: naiveDateOfString(str: _dateFormat.format(to)),
      );
      if (mounted) widget.onJourneyRangeLoaded(proxy);
    } finally {
      if (mounted) setState(() => _loading = false);
    }
  }

  void _onLayerFilterChanged() {
    if (_lastFrom != null && _lastTo != null) {
      _loadJourneyForRange(_lastFrom!, _lastTo!);
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
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Spacer(),
            Padding(
              padding: EdgeInsets.only(
                bottom: isLandscape ? 40 : screenSize.height * 0.12,
              ),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  _TimeMachineLayerButton(onLayerFilterChanged: _onLayerFilterChanged),
                  const SizedBox(height: 12),
                  TimeRangePicker(
                    earliestDate: earliest,
                    loading: _loading,
                    onRangeChanged: _loadJourneyForRange,
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Layer button matching [TimeRangeControllerBall] style (60x60, frosted glass, radius 12).
/// Uses main map layer filter; changing it reloads the time range with the same filter.
class _TimeMachineLayerButton extends StatelessWidget {
  const _TimeMachineLayerButton({
    required this.onLayerFilterChanged,
  });

  final VoidCallback onLayerFilterChanged;

  static const double _buttonSize = 60;
  static const double _borderRadius = 12;

  @override
  Widget build(BuildContext context) {
    return CustomPopup(
      position: PopupPosition.top,
      verticalOffset: 12,
      contentRadius: 12,
      barrierColor: Colors.transparent,
      content: PointerInterceptor(
        child: _TimeMachineLayerPopupContent(onLayerFilterChanged: onLayerFilterChanged),
      ),
      child: PointerInterceptor(
        child: ClipRRect(
          borderRadius: BorderRadius.circular(_borderRadius),
          child: BackdropFilter(
            filter: ImageFilter.blur(sigmaX: 8, sigmaY: 8),
            child: Container(
              width: _buttonSize,
              height: _buttonSize,
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: 0.2),
                borderRadius: BorderRadius.circular(_borderRadius),
                border: Border.all(
                  color: Colors.white.withValues(alpha: 0.35),
                  width: 1,
                ),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withValues(alpha: 0.12),
                    blurRadius: 12,
                    offset: const Offset(0, 2),
                  ),
                ],
              ),
              child: Icon(
                Icons.layers,
                color: Colors.white,
                size: 24,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _TimeMachineLayerPopupContent extends StatefulWidget {
  const _TimeMachineLayerPopupContent({
    required this.onLayerFilterChanged,
  });

  final VoidCallback onLayerFilterChanged;

  @override
  State<_TimeMachineLayerPopupContent> createState() => _TimeMachineLayerPopupContentState();
}

class _TimeMachineLayerPopupContentState extends State<_TimeMachineLayerPopupContent> {
  api.LayerFilter _layerFilter = api.getCurrentMainMapLayerFilter();
  Timer? _actionTimer;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildItem(LayerOption.current, context.tr('journey_kind.current'), FontAwesomeIcons.locationDot),
        _buildItem(LayerOption.default_, context.tr('journey_kind.default'), FontAwesomeIcons.shoePrints),
        _buildItem(LayerOption.flight, context.tr('journey_kind.flight'), FontAwesomeIcons.planeUp),
      ],
    );
  }

  Widget _buildItem(LayerOption layerOption, String text, IconData icon) {
    final isActive = switch (layerOption) {
      LayerOption.current => _layerFilter.currentJourney,
      LayerOption.default_ => _layerFilter.defaultKind,
      LayerOption.flight => _layerFilter.flightKind,
    };

    return InkWell(
      onTap: () {
        setState(() {
          switch (layerOption) {
            case LayerOption.current:
              _layerFilter.currentJourney = !_layerFilter.currentJourney;
            case LayerOption.default_:
              _layerFilter.defaultKind = !_layerFilter.defaultKind;
            case LayerOption.flight:
              _layerFilter.flightKind = !_layerFilter.flightKind;
          }
        });
        _actionTimer?.cancel();
        _actionTimer = Timer(const Duration(milliseconds: 600), () {
          _actionTimer = null;
          api.setMainMapLayerFilter(newLayerFilter: _layerFilter);
          widget.onLayerFilterChanged();
        });
      },
      borderRadius: BorderRadius.circular(8),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 10, horizontal: 12),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(
              icon,
              color: isActive ? StyleConstants.defaultColor : Colors.white70,
              size: 16,
            ),
            const SizedBox(width: 8),
            Text(
              text,
              style: TextStyle(
                color: isActive ? StyleConstants.defaultColor : Colors.white70,
                fontSize: 14,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

enum LayerOption {
  current,
  default_,
  flight,
}
