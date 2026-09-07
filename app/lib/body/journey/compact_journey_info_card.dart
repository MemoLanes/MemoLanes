import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/common/simple_date_utils.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:memolanes/src/rust/journey_header.dart';

class JourneyInfoPanelSurface extends StatelessWidget {
  const JourneyInfoPanelSurface({
    super.key,
    required this.child,
    this.backgroundAlpha = 0.84,
  });

  final Widget child;
  final double backgroundAlpha;

  @override
  Widget build(BuildContext context) {
    return AppDialogSurface(
      style: AppDialogSurfaceStyle.glass,
      glassBackgroundAlpha: backgroundAlpha,
      shadowAlpha: StyleConstants.mapOverlayShadowAlpha,
      shadowBlurRadius: StyleConstants.mapOverlayShadowBlurRadius,
      shadowSpreadRadius: StyleConstants.mapOverlayShadowSpreadRadius,
      shadowOffset: StyleConstants.mapOverlayShadowOffset,
      child: child,
    );
  }
}

class JourneyInfoCardHeader extends StatelessWidget {
  const JourneyInfoCardHeader({super.key});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        Container(
          width: 30,
          height: 30,
          decoration: BoxDecoration(
            color: StyleConstants.softGreen.withValues(alpha: 0.8),
            borderRadius: BorderRadius.circular(10),
          ),
          child: Icon(
            Icons.route_rounded,
            size: 17,
            color: StyleConstants.deepGreen,
          ),
        ),
        const SizedBox(width: 9),
        Expanded(
          child: Text(
            context.tr('journey.journey_info_page_title'),
            style: AppTypography.subpageTitle.copyWith(
              color: StyleConstants.deepGreen,
            ),
          ),
        ),
      ],
    );
  }
}

class CompactJourneyInfoField extends StatelessWidget {
  const CompactJourneyInfoField({
    super.key,
    required this.icon,
    required this.label,
    this.value,
    this.trailing,
    this.onTap,
    this.maxLines = 1,
  }) : assert(value != null || trailing != null);

  final IconData icon;
  final String label;
  final String? value;
  final Widget? trailing;
  final VoidCallback? onTap;
  final int maxLines;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(11),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 5, vertical: 5),
          child: Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Padding(
                padding: const EdgeInsets.only(top: 1),
                child: Icon(
                  icon,
                  size: 15,
                  color: StyleConstants.mutedInkColor,
                ),
              ),
              const SizedBox(width: 8),
              SizedBox(
                width: 92,
                child: Text(
                  label,
                  style: AppTypography.label.copyWith(
                    color: StyleConstants.mutedInkColor,
                  ),
                ),
              ),
              const SizedBox(width: 6),
              Expanded(
                child:
                    trailing ??
                    Row(
                      mainAxisAlignment: MainAxisAlignment.end,
                      children: [
                        Flexible(
                          child: Text(
                            value!,
                            maxLines: maxLines,
                            overflow: TextOverflow.ellipsis,
                            textAlign: TextAlign.right,
                            style: AppTypography.supporting.copyWith(
                              color: StyleConstants.deepGreen,
                              fontWeight: FontWeight.w600,
                            ),
                          ),
                        ),
                        if (onTap != null) ...[
                          const SizedBox(width: 3),
                          Icon(
                            Icons.chevron_right_rounded,
                            size: 17,
                            color: StyleConstants.deepGreen,
                          ),
                        ],
                      ],
                    ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Read-only journey information using the same floating card primitives as
/// the editable journey card on the map.
class ReadOnlyJourneyInfoCard extends StatelessWidget {
  const ReadOnlyJourneyInfoCard({super.key, required this.journey});

  final JourneyHeader journey;

  @override
  Widget build(BuildContext context) {
    final kind = switch (journey.journeyKind) {
      JourneyKind.defaultKind => context.tr('journey_kind.default'),
      JourneyKind.flight => context.tr('journey_kind.flight'),
    };
    final timeFormat = DateFormat('yyyy-MM-dd HH:mm');
    final start = journey.start?.toLocal();
    final end = journey.end?.toLocal();
    final note = journey.note?.trim();

    return JourneyInfoPanelSurface(
      child: Padding(
        padding: const EdgeInsets.fromLTRB(14, 10, 14, 13),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const JourneyInfoCardHeader(),
            const SizedBox(height: 5),
            CompactJourneyInfoField(
              icon: Icons.calendar_today_rounded,
              label: context.tr('journey.journey_date'),
              value: journey.journeyDate.toSimpleDate().toString(),
            ),
            CompactJourneyInfoField(
              icon: Icons.sell_outlined,
              label: context.tr('journey.journey_kind'),
              value: kind,
            ),
            CompactJourneyInfoField(
              icon: Icons.schedule_rounded,
              label: context.tr('journey.start_time'),
              value: start == null ? '—' : timeFormat.format(start),
            ),
            CompactJourneyInfoField(
              icon: Icons.schedule_rounded,
              label: context.tr('journey.end_time'),
              value: end == null ? '—' : timeFormat.format(end),
            ),
            if (note != null && note.isNotEmpty)
              CompactJourneyInfoField(
                icon: Icons.notes_rounded,
                label: context.tr('journey.note'),
                value: note,
                maxLines: 2,
              ),
          ],
        ),
      ),
    );
  }
}
