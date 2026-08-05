import 'dart:async';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:font_awesome_flutter/font_awesome_flutter.dart';
import 'package:memolanes/common/app_haptics.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class LayerButton extends StatelessWidget {
  const LayerButton({
    super.key,
  });

  @override
  Widget build(BuildContext context) {
    return CustomPopup(
      theme: CustomPopupTheme.white,
      position: PopupPosition.left,
      horizontalOffset: -16,
      contentRadius: 24,
      content: PointerInterceptor(child: const LayerPopupContent()),
      builder: (context, show) => PointerInterceptor(
        child: Material(
          color: Colors.transparent,
          shape: const CircleBorder(),
          child: InkWell(
            onTap: () {
              AppHaptics.selection();
              show();
            },
            customBorder: const CircleBorder(),
            child: Container(
              width: 48,
              height: 48,
              decoration: const BoxDecoration(
                color: StyleConstants.glassControlSurfaceColor,
                shape: BoxShape.circle,
                border: Border.fromBorderSide(
                  BorderSide(color: StyleConstants.glassControlBorderColor),
                ),
              ),
              child: const Icon(
                Icons.layers,
                color: StyleConstants.glassControlContentColor,
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
            FontAwesomeIcons.shoePrints),
        _buildItem(LayerOption.flight, context.tr("journey_kind.flight"),
            FontAwesomeIcons.planeUp),
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
                      ? CustomPopupTheme.white.accentColor
                      : CustomPopupTheme.white.mutedContentColor,
                  size: 16,
                ),
              ),
            ),
            const SizedBox(width: 8),
            Text(
              text,
              style: TextStyle(
                color: isActive
                    ? CustomPopupTheme.white.accentColor
                    : CustomPopupTheme.white.mutedContentColor,
                fontSize: 14,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
