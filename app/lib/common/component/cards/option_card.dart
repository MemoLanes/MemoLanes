import 'package:flutter/material.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';
import 'package:memolanes/constants/style_constants.dart';

class OptionCard extends StatelessWidget {
  const OptionCard({
    super.key,
    required this.children,
    this.useSafeArea = true,
    this.separators = true,
    this.embedded = false,
  });

  final List<Widget> children;
  final bool useSafeArea;
  final bool separators;
  final bool embedded;

  @override
  Widget build(BuildContext context) {
    final card = Container(
      decoration: embedded
          ? null
          : BoxDecoration(
              color: StyleConstants.surfaceColor,
              borderRadius: BorderRadius.circular(16.0),
              border: Border.all(color: StyleConstants.lineColor),
              boxShadow: [
                BoxShadow(
                  color: StyleConstants.shadowColor.withValues(
                    alpha: StyleConstants.isDarkMode ? 0.34 : 0.055,
                  ),
                  blurRadius: 20,
                  offset: const Offset(0, 7),
                ),
              ],
            ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(16.0),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: separators ? _withDividers(children) : children,
        ),
      ),
    );

    if (!useSafeArea) return card;
    return SafeAreaWrapper(child: card);
  }

  List<Widget> _withDividers(List<Widget> widgets) {
    if (widgets.isEmpty) return [];
    final List<Widget> result = [];
    for (int i = 0; i < widgets.length; i++) {
      result.add(widgets[i]);
      if (i != widgets.length - 1) {
        result.add(Container(height: 0.5, color: StyleConstants.lineColor));
      }
    }
    return result;
  }
}
