import 'dart:math' as math;

import 'package:memolanes/theme/app_colors.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/body/journey/journey_body.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/utils.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/utils/nav_helper.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

import 'journey_map_detail_page.dart';

/// Journeys keeps the current map visible without Record mode's three map
/// controls. Its picker is an in-page floating card, so the app navigation bar
/// remains visible and unobstructed below it. Selecting a journey pushes a
/// dedicated map-detail route where the app navigation bar is intentionally
/// absent.
class JourneyOverlay extends StatefulWidget {
  const JourneyOverlay({super.key});

  @override
  State<JourneyOverlay> createState() => _JourneyOverlayState();
}

class _JourneyOverlayState extends State<JourneyOverlay> {
  bool _isLoadingJourney = false;
  int _pickerRevision = 0;

  Future<void> _openJourneyDetails(JourneyHeader journey) async {
    if (_isLoadingJourney) return;
    setState(() => _isLoadingJourney = true);

    try {
      final rendererAndBounds = await api.getMapRendererProxyForJourney(
        journeyId: journey.id,
      );
      if (!mounted) return;
      await navigatorPush<void>(
        context,
        page: JourneyMapDetailPage(
          journey: journey,
          mapRendererProxy: rendererAndBounds.$1,
          initialMapBounds: rendererAndBounds.$2,
        ),
      );
      if (!mounted) return;
      setState(() => _pickerRevision++);
    } catch (error, stackTrace) {
      log.error('Loading journey map failed: $error', stackTrace);
      if (!mounted) return;
      await showCommonDialog(
        context,
        context.tr('journey.editor.operation_failed'),
      );
    } finally {
      if (mounted) setState(() => _isLoadingJourney = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final mediaQuery = MediaQuery.of(context);
    final viewPadding = mediaQuery.viewPadding;
    final bottom = StyleConstants.mapPrimaryControlBottomInsetForContext(
      context,
    );
    final availableHeight = math.max(
      0.0,
      mediaQuery.size.height - bottom - viewPadding.top - 12,
    );
    final preferredHeight = (mediaQuery.size.height * 0.62)
        .clamp(390.0, 530.0)
        .toDouble();
    final pickerHeight = math.min(preferredHeight, availableHeight);
    final isLandscape = mediaQuery.orientation == Orientation.landscape;
    final pickerMaxWidth = isLandscape ? 720.0 : 480.0;

    return Stack(
      children: [
        Positioned(
          left: viewPadding.left + 16,
          right: viewPadding.right + 16,
          bottom: bottom,
          child: Align(
            alignment: Alignment.bottomCenter,
            child: SizedBox(
              width: math.min(
                mediaQuery.size.width -
                    viewPadding.left -
                    viewPadding.right -
                    32,
                pickerMaxWidth,
              ),
              height: pickerHeight,
              child: PointerInterceptor(
                child: _JourneyPickerCard(
                  onJourneySelected: _openJourneyDetails,
                  isLoading: _isLoadingJourney,
                  refreshRevision: _pickerRevision,
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _JourneyPickerCard extends StatelessWidget {
  const _JourneyPickerCard({
    required this.onJourneySelected,
    required this.isLoading,
    required this.refreshRevision,
  });

  final Future<void> Function(JourneyHeader journey) onJourneySelected;
  final bool isLoading;
  final int refreshRevision;

  @override
  Widget build(BuildContext context) {
    return JourneyInfoPanelSurface(
      backgroundAlpha: 0.76,
      child: Stack(
        children: [
          Column(
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(18, 10, 9, 8),
                child: Row(
                  children: [
                    Container(
                      width: 30,
                      height: 30,
                      decoration: BoxDecoration(
                        color: context.appColors.softGreen,
                        shape: BoxShape.circle,
                      ),
                      child: Icon(
                        Icons.route_rounded,
                        size: 17,
                        color: context.appColors.deepGreen,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: Text(
                        context.tr('journey.picker_title'),
                        style: AppTypography.surfaceTitle.copyWith(
                          color: context.appColors.deepGreen,
                        ),
                      ),
                    ),
                    const SizedBox(width: 9),
                  ],
                ),
              ),
              Divider(
                height: 1,
                color: context.appColors.lineColor.withValues(alpha: 0.9),
              ),
              Expanded(
                child: JourneyBody(
                  onJourneySelected: onJourneySelected,
                  refreshRevision: refreshRevision,
                ),
              ),
            ],
          ),
          if (isLoading)
            Positioned.fill(
              child: ColoredBox(
                color: context.appColors.surfaceColor.withValues(alpha: 0.42),
                child: Center(
                  child: CircularProgressIndicator(
                    color: context.appColors.deepGreen,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}
