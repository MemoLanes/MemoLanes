import 'package:flutter/material.dart';
import 'package:memolanes/common/component/app_dialog.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/constants/style_constants.dart';

Future<T?> showSetupCard<T>(
  BuildContext context, {
  required WidgetBuilder builder,
  bool barrierDismissible = true,
  Color? barrierColor,
}) {
  return showAppDialog<T>(
    context,
    barrierDismissible: barrierDismissible,
    barrierColor: barrierColor,
    maxWidth: 440,
    builder: builder,
  );
}

class SetupDialogCard extends StatelessWidget {
  const SetupDialogCard({
    super.key,
    required this.title,
    required this.child,
    this.actions = const [],
    this.leading,
    this.showTitle = true,
    this.maxHeightFactor = 0.75,
    this.contentPadding = const EdgeInsets.symmetric(
      horizontal: 20,
      vertical: 4,
    ),
  });

  final String title;
  final Widget child;
  final List<Widget> actions;
  final Widget? leading;
  final bool showTitle;
  final double maxHeightFactor;
  final EdgeInsetsGeometry contentPadding;

  @override
  Widget build(BuildContext context) {
    return AppDialogCard(
      title: title,
      leading: leading,
      showHeader: showTitle,
      maxHeightFactor: maxHeightFactor,
      contentPadding: contentPadding,
      actions: actions.isEmpty
          ? null
          : AppDialogActions(spacing: 10, children: actions),
      child: child,
    );
  }
}

@Deprecated('Use SetupDialogCard; UI v2 presents a centered dialog card.')
class SetupBottomSheet extends SetupDialogCard {
  const SetupBottomSheet({
    super.key,
    required super.title,
    required super.child,
    super.actions,
    super.leading,
    super.showTitle,
    super.maxHeightFactor,
    super.contentPadding,
  });
}

class SetupTile extends StatelessWidget {
  const SetupTile({
    super.key,
    required this.icon,
    required this.title,
    this.subtitle,
    this.trailing,
    this.titleTrailing,
    this.extraContent,
    this.onTap,
    this.selected = false,
    this.minHeight,
    this.contentPadding = const EdgeInsets.symmetric(
      horizontal: 12,
      vertical: 10,
    ),
  });

  final IconData icon;
  final String title;
  final String? subtitle;
  final Widget? trailing;
  final Widget? titleTrailing;
  final Widget? extraContent;
  final VoidCallback? onTap;
  final bool selected;
  final double? minHeight;
  final EdgeInsetsGeometry contentPadding;

  @override
  Widget build(BuildContext context) {
    final tile = Container(
      constraints: minHeight == null
          ? null
          : BoxConstraints(minHeight: minHeight!),
      padding: contentPadding,
      decoration: BoxDecoration(
        color: selected
            ? StyleConstants.softGreen.withValues(alpha: 0.82)
            : StyleConstants.surfaceColor,
        borderRadius: BorderRadius.circular(14),
        border: Border.all(
          color: selected
              ? StyleConstants.primaryGreen
              : StyleConstants.lineColor,
          width: selected ? 1.4 : 1,
        ),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Container(
            width: 34,
            height: 34,
            decoration: BoxDecoration(
              color: selected
                  ? StyleConstants.primaryGreen.withValues(alpha: 0.32)
                  : StyleConstants.softGreen,
              borderRadius: BorderRadius.circular(10),
            ),
            alignment: Alignment.center,
            child: Icon(icon, color: StyleConstants.deepGreen, size: 20),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Row(
                  crossAxisAlignment: CrossAxisAlignment.center,
                  children: [
                    Flexible(
                      child: Text(
                        title,
                        style: AppTypography.cardTitle.copyWith(
                          color: StyleConstants.inkColor,
                        ),
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                      ),
                    ),
                    if (titleTrailing != null) ...[
                      const SizedBox(width: 6),
                      titleTrailing!,
                    ],
                  ],
                ),
                if (subtitle != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 2),
                    child: Text(
                      subtitle!,
                      style: AppTypography.caption.copyWith(
                        color: StyleConstants.mutedInkColor,
                      ),
                    ),
                  ),
                ?extraContent,
              ],
            ),
          ),
          ?trailing,
        ],
      ),
    );

    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(14),
        child: tile,
      ),
    );
  }
}
