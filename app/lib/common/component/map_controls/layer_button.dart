import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/common/component/liquid_glass_surface.dart';
import 'package:memolanes/common/journey_kind_visuals.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';

class LayerButton extends StatelessWidget {
  const LayerButton({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    return CustomPopup(
      position: PopupPosition.left,
      horizontalOffset: -16,
      contentRadius: 24,
      barrierColor: Colors.transparent,
      contentDecoration: BoxDecoration(
        color: StyleConstants.glassColor.withValues(
          alpha: StyleConstants.isDarkMode ? 0.94 : 0.68,
        ),
        borderRadius: BorderRadius.circular(24),
        border: Border.all(
          color: StyleConstants.glassBorderColor.withValues(
            alpha: StyleConstants.isDarkMode ? 0.48 : 0.8,
          ),
        ),
        boxShadow: [
          BoxShadow(
            color: StyleConstants.shadowColor.withValues(
              alpha: StyleConstants.isDarkMode ? 0.48 : 0.14,
            ),
            blurRadius: 22,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      content: PointerInterceptor(child: const LayerPopupContent()),
      child: PointerInterceptor(
        child: LiquidGlassSurface(
          circular: true,
          child: SizedBox(
            width: 44,
            height: 44,
            child: Center(
              child: Icon(
                Icons.layers,
                color: StyleConstants.deepGreen,
                size: 20,
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class LayerPopupContent extends StatefulWidget {
  const LayerPopupContent({
    super.key,
  });

  @override
  State<LayerPopupContent> createState() => _LayerPopupContentState();
}

enum LayerOption {
  current,
  default_,
  flight,
}

class _LayerPopupContentState extends State<LayerPopupContent> {
  final api.LayerFilter _layerFilter = api.getCurrentMainMapLayerFilter();
  Timer? _actionTimer;

  @override
  void initState() {
    super.initState();
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _buildItem(LayerOption.current, context.tr("journey_kind.current"),
            FontAwesomeIcons.locationDot),
        _buildItem(LayerOption.default_, context.tr("journey_kind.default"),
            journeyKindIconData(JourneyKind.defaultKind)),
        _buildItem(LayerOption.flight, context.tr("journey_kind.flight"),
            journeyKindIconData(JourneyKind.flight)),
      ],
    );
  }

  Widget _buildItem(LayerOption layerOption, String text, FaIconData icon) {
    final isActive = switch (layerOption) {
      LayerOption.current => _layerFilter.currentJourney,
      LayerOption.default_ => _layerFilter.defaultKind,
      LayerOption.flight => _layerFilter.flightKind,
    };

    return InkWell(
      onTap: () {
        AppHaptics.selection();
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
        });
      },
      borderRadius: BorderRadius.circular(12),
      child: Padding(
        padding: const EdgeInsets.symmetric(vertical: 6, horizontal: 8),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            SizedBox(
              width: 20,
              child: Center(
                child: FaIcon(
                  icon,
                  color: isActive
                      ? StyleConstants.deepGreen
                      : StyleConstants.mutedInkColor,
                  size: 16,
                ),
              ),
            ),
            const SizedBox(width: 8),
            Text(
              text,
              style: AppTypography.itemTitle.copyWith(
                color: isActive
                    ? StyleConstants.deepGreen
                    : StyleConstants.mutedInkColor,
                fontWeight: isActive ? FontWeight.w700 : FontWeight.w600,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
