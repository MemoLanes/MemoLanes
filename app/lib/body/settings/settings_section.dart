import 'package:flutter/material.dart';
import 'package:memolanes/common/component/cards/option_card.dart';
import 'package:memolanes/common/component/scroll_views/single_child_scroll_view.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/theme/app_colors.dart';

class SettingsPageFrame extends StatelessWidget {
  const SettingsPageFrame({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.symmetric(horizontal: 16),
  });

  static const double maxContentWidth = 640;

  final Widget child;
  final EdgeInsetsGeometry padding;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: padding,
      child: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: maxContentWidth),
          child: SizedBox(width: double.infinity, child: child),
        ),
      ),
    );
  }
}

class SettingsPageLayout extends StatelessWidget {
  const SettingsPageLayout({
    super.key,
    required this.children,
    this.topPadding = 0,
    this.bottomPadding = 24,
    this.framePadding = const EdgeInsets.symmetric(horizontal: 16),
  });

  final List<Widget> children;
  final double topPadding;
  final double bottomPadding;
  final EdgeInsetsGeometry framePadding;

  @override
  Widget build(BuildContext context) {
    return MlSingleChildScrollView(
      padding: EdgeInsets.only(top: topPadding, bottom: bottomPadding),
      children: [
        SettingsPageFrame(
          padding: framePadding,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: children,
          ),
        ),
      ],
    );
  }
}

class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.title,
    required this.children,
  });

  final String title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: double.infinity,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.fromLTRB(8, 12, 8, 6),
            child: Text(
              title,
              style: AppTypography.sectionLabel.copyWith(
                color: context.appColors.mutedInkColor,
              ),
            ),
          ),
          OptionCard(useSafeArea: false, separators: false, children: children),
          const SizedBox(height: 4),
        ],
      ),
    );
  }
}
