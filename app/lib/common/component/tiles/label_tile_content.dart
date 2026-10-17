import 'package:memolanes/theme/app_colors.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/constants/app_typography.dart';

class LabelTileContent extends StatelessWidget {
  const LabelTileContent({
    super.key,
    this.content = '',
    this.contentMaxLines = 1,
    this.showArrow = false,
    this.rightIcon,
    this.rightIconColor,
  });

  final String content;

  final int contentMaxLines;

  final bool showArrow;

  final IconData? rightIcon;

  final Color? rightIconColor;

  Widget _buildContent(BuildContext context) {
    return Text(
      content,
      style: AppTypography.body.copyWith(
        color: context.appColors.mutedInkColor,
      ),
      textAlign: TextAlign.end,
      maxLines: contentMaxLines,
      overflow: TextOverflow.ellipsis,
    );
  }

  Widget? _buildIcon(BuildContext context) {
    final IconData? effectiveIcon =
        rightIcon ?? (showArrow ? Icons.arrow_forward_ios : null);
    if (effectiveIcon == null) return null;

    return Icon(
      effectiveIcon,
      size: 16.0,
      color: rightIconColor ?? context.appColors.mutedInkColor,
    );
  }

  @override
  Widget build(BuildContext context) {
    final icon = _buildIcon(context);
    return Row(
      mainAxisSize: MainAxisSize.min,
      crossAxisAlignment: CrossAxisAlignment.center,
      children: [
        Flexible(child: _buildContent(context)),
        if (icon != null) ...[const SizedBox(width: 8), icon],
      ],
    );
  }
}
