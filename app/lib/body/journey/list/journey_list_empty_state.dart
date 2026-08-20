import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';

enum JourneyListEmptyType { all, filtered, month }

class JourneyListEmptyState extends StatelessWidget {
  final JourneyListEmptyType type;
  final bool topAligned;
  final VoidCallback? onShowAll;

  const JourneyListEmptyState({
    super.key,
    required this.type,
    this.topAligned = false,
    this.onShowAll,
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
        padding: EdgeInsets.fromLTRB(24, topAligned ? 24 : 32, 24, 24),
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 320),
          child: Container(
            width: double.infinity,
            padding: const EdgeInsets.all(24),
            decoration: BoxDecoration(
              color: Colors.white.withValues(alpha: 0.05),
              borderRadius: BorderRadius.circular(20),
              border: Border.all(color: Colors.white.withValues(alpha: 0.08)),
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 48,
                  height: 48,
                  decoration: BoxDecoration(
                    color: StyleConstants.defaultColor.withValues(alpha: 0.14),
                    shape: BoxShape.circle,
                  ),
                  child: Icon(
                    icon,
                    color: StyleConstants.defaultColor,
                    size: 25,
                  ),
                ),
                const SizedBox(height: 14),
                Text(
                  context.tr(titleKey),
                  textAlign: TextAlign.center,
                  style: const TextStyle(
                    color: Colors.white,
                    fontSize: 16,
                    fontWeight: FontWeight.w600,
                  ),
                ),
                const SizedBox(height: 6),
                Text(
                  context.tr(descriptionKey),
                  textAlign: TextAlign.center,
                  style: const TextStyle(color: Colors.white60, height: 1.45),
                ),
                if (onShowAll != null) ...[
                  const SizedBox(height: 12),
                  FilledButton.icon(
                    onPressed: onShowAll,
                    style: FilledButton.styleFrom(
                      backgroundColor: StyleConstants.defaultColor,
                      foregroundColor: Colors.black,
                      padding: const EdgeInsets.symmetric(
                        horizontal: 16,
                        vertical: 10,
                      ),
                      shape: RoundedRectangleBorder(
                        borderRadius: BorderRadius.circular(10),
                      ),
                    ),
                    icon: const Icon(Icons.layers_outlined, size: 18),
                    label: Text(context.tr('journey.list.show_all_layers')),
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
