import 'package:memolanes/theme/app_colors.dart';

import 'package:easy_localization/easy_localization.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/map_overlay_panel.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/body/journey/journey_body.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// Journeys keeps the current map visible without Record mode's three map
/// controls. Its picker is an in-page floating card, so the app navigation bar
/// remains visible and unobstructed below it.
class JourneyOverlay extends StatelessWidget {
  const JourneyOverlay({
    super.key,
    required this.onJourneySelected,
    required this.isLoading,
    required this.refreshRevision,
    this.detail,
  });

  final Future<void> Function(JourneyHeader journey) onJourneySelected;
  final bool isLoading;
  final int refreshRevision;
  final Widget? detail;

  @override
  Widget build(BuildContext context) => JourneyPickerOverlay(
    isLoading: isLoading,
    detail: detail,
    child: JourneyBody(
      onJourneySelected: onJourneySelected,
      refreshRevision: refreshRevision,
    ),
  );
}

class JourneyPickerOverlay extends StatelessWidget {
  const JourneyPickerOverlay({
    super.key,
    required this.child,
    this.isLoading = false,
    this.detail,
  });

  final Widget child;
  final bool isLoading;
  final Widget? detail;

  @override
  Widget build(BuildContext context) {
    return Stack(
      children: [
        Offstage(
          offstage: detail != null,
          child: Stack(
            children: [
              MapOverlayPanel.picker(
                child: PointerInterceptor(
                  child: _JourneyPickerCard(isLoading: isLoading, child: child),
                ),
              ),
            ],
          ),
        ),
        ?detail,
      ],
    );
  }
}

class _JourneyPickerCard extends StatelessWidget {
  const _JourneyPickerCard({required this.isLoading, required this.child});

  final bool isLoading;
  final Widget child;

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
              Flexible(child: child),
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
