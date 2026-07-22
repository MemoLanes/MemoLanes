import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/line_painter.dart';
import 'package:memolanes/constants/style_constants.dart';

class SetupBottomSheet extends StatelessWidget {
  const SetupBottomSheet({
    super.key,
    required this.title,
    required this.child,
    this.actions = const [],
    this.leading,
    this.showTitle = true,
    this.maxHeightFactor = 0.75,
    this.contentPadding =
        const EdgeInsets.symmetric(horizontal: 20, vertical: 4),
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
    return Container(
      constraints: BoxConstraints(
        maxHeight: MediaQuery.of(context).size.height * maxHeightFactor,
      ),
      decoration: const BoxDecoration(
        color: Colors.black,
        borderRadius: BorderRadius.only(
          topLeft: Radius.circular(16.0),
          topRight: Radius.circular(16.0),
        ),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 8.0),
            child: Center(
              child: CustomPaint(
                size: const Size(40.0, 4.0),
                painter: LinePainter(color: const Color(0xFFB5B5B5)),
              ),
            ),
          ),
          if (showTitle)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 4.0, vertical: 0),
              child: Row(
                children: [
                  leading ?? const SizedBox(width: 48),
                  Expanded(
                    child: Text(
                      title,
                      style: const TextStyle(
                        color: Colors.white,
                        fontSize: 16,
                        fontWeight: FontWeight.w600,
                      ),
                      textAlign: TextAlign.center,
                    ),
                  ),
                  const SizedBox(width: 48),
                ],
              ),
            ),
          Flexible(
            child: SingleChildScrollView(
              padding: contentPadding,
              child: child,
            ),
          ),
          if (actions.isNotEmpty)
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 10, 20, 20),
              child: Row(
                children: [
                  for (var i = 0; i < actions.length; i++) ...[
                    if (i > 0) const SizedBox(width: 12),
                    Expanded(child: actions[i]),
                  ],
                ],
              ),
            ),
        ],
      ),
    );
  }
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
    this.contentPadding =
        const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
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
      constraints:
          minHeight == null ? null : BoxConstraints(minHeight: minHeight!),
      padding: contentPadding,
      decoration: BoxDecoration(
        color: const Color(0x1AFFFFFF),
        borderRadius: BorderRadius.circular(10),
        border: selected
            ? Border.all(color: StyleConstants.defaultColor)
            : Border.all(color: Colors.transparent),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.center,
        children: [
          Icon(
            icon,
            color: StyleConstants.defaultColor,
            size: 22,
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
                        style: const TextStyle(
                          color: Colors.white,
                          fontSize: 15,
                          fontWeight: FontWeight.w500,
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
                      style: const TextStyle(
                        color: Color(0xFFB0B0B0),
                        fontSize: 12,
                      ),
                    ),
                  ),
                if (extraContent != null) extraContent!,
              ],
            ),
          ),
          if (trailing != null) trailing!,
        ],
      ),
    );

    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(10),
        child: tile,
      ),
    );
  }
}
