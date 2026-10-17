import 'dart:ui' show lerpDouble;

import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/theme/app_colors.dart';

class OptionCard extends StatelessWidget {
  const OptionCard({
    super.key,
    required this.children,
    this.useSafeArea = true,
    this.separators = true,
    this.embedded = false,
  }) : contentBuilder = null;

  const OptionCard.builder({
    super.key,
    required this.contentBuilder,
    this.useSafeArea = true,
    this.separators = true,
    this.embedded = false,
  }) : children = const [];

  final List<Widget> children;
  final List<Widget> Function(BuildContext, BoxConstraints)? contentBuilder;
  final bool useSafeArea;
  final bool separators;
  final bool embedded;

  @override
  Widget build(BuildContext context) {
    final builder = contentBuilder;
    final card = Container(
      decoration: embedded
          ? null
          : BoxDecoration(
              color: context.appColors.surfaceColor,
              borderRadius: BorderRadius.circular(16.0),
              border: Border.all(color: context.appColors.lineColor),
              boxShadow: [
                BoxShadow(
                  color: context.appColors.shadowColor.withValues(
                    alpha: lerpDouble(
                      0.055,
                      0.34,
                      context.appColors.darkProgress,
                    )!,
                  ),
                  blurRadius: 20,
                  offset: const Offset(0, 7),
                ),
              ],
            ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(16.0),
        child: builder == null
            ? _buildColumn(context, children)
            : LayoutBuilder(
                builder: (context, constraints) =>
                    _buildColumn(context, builder(context, constraints)),
              ),
      ),
    );

    if (!useSafeArea) return card;
    return SafeAreaWrapper(child: card);
  }

  Widget _buildColumn(BuildContext context, List<Widget> children) => Column(
    mainAxisSize: MainAxisSize.min,
    children: separators ? _withDividers(context, children) : children,
  );

  List<Widget> _withDividers(BuildContext context, List<Widget> widgets) {
    if (widgets.isEmpty) return [];
    final List<Widget> result = [];
    for (int i = 0; i < widgets.length; i++) {
      result.add(widgets[i]);
      if (i != widgets.length - 1) {
        result.add(Container(height: 0.5, color: context.appColors.lineColor));
      }
    }
    return result;
  }
}
