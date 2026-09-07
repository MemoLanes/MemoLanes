import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

enum JourneyListEmptyType { all, filtered, month }

class JourneyListEmptyState extends StatelessWidget {
  final JourneyListEmptyType type;
  final bool topAligned;
  final VoidCallback? onShowAll;
  final bool compact;

  const JourneyListEmptyState({
    super.key,
    required this.type,
    this.topAligned = false,
    this.onShowAll,
    this.compact = false,
  });

  @override
  Widget build(BuildContext context) {
    final (titleKey, descriptionKey, icon) = switch (type) {
      JourneyListEmptyType.all => (
        'journey.list.empty_all_title',
        'journey.list.empty_all_description',
        Icons.explore_outlined,
      ),
      JourneyListEmptyType.filtered => (
        'journey.list.empty_filtered_title',
        'journey.list.empty_filtered_description',
        Icons.layers_clear_outlined,
      ),
      JourneyListEmptyType.month => (
        'journey.list.empty_month_title',
        'journey.list.empty_month_description',
        Icons.calendar_month_outlined,
      ),
    };

    return Align(
      alignment: topAligned ? Alignment.topCenter : Alignment.center,
      child: Padding(
        padding: EdgeInsets.fromLTRB(
          compact ? 8 : 24,
          topAligned ? (compact ? 10 : 24) : 32,
          compact ? 8 : 24,
          compact ? 12 : 24,
        ),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 320),
          child: Container(
            width: double.infinity,
            padding: EdgeInsets.all(compact ? 18 : 24),
            decoration: BoxDecoration(
              color: StyleConstants.surfaceColor,
              borderRadius: BorderRadius.circular(20),
              border: Border.all(color: StyleConstants.lineColor),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 48,
                  height: 48,
                  decoration: BoxDecoration(
                    color: StyleConstants.softYellow,
                    shape: BoxShape.circle,
                  ),
                  child: Icon(icon, color: StyleConstants.deepYellow, size: 25),
                ),
                const SizedBox(height: 14),
                Text(
                  context.tr(titleKey),
                  textAlign: TextAlign.center,
                  style: AppTypography.subpageTitle.copyWith(
                    color: StyleConstants.inkColor,
                  ),
                ),
                const SizedBox(height: 6),
                Text(
                  context.tr(descriptionKey),
                  textAlign: TextAlign.center,
                  style: AppTypography.supporting.copyWith(
                    color: StyleConstants.mutedInkColor,
                  ),
                ),
                if (onShowAll != null) ...[
                  const SizedBox(height: 12),
                  AppButton(
                    onPressed: onShowAll,
                    icon: Icons.layers_outlined,
                    size: compact
                        ? AppButtonSize.compact
                        : AppButtonSize.regular,
                    label: context.tr('journey.list.show_all_layers'),
                  ),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}
