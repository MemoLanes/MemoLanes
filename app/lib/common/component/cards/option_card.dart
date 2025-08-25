import 'package:flutter/material.dart';
import 'package:memolanes/common/component/safe_area_wrapper.dart';

class OptionCard extends StatelessWidget {
  const OptionCard({
    super.key,
    required this.children,
  });

  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return SafeAreaWrapper(
      child: Container(
        decoration: BoxDecoration(
          color: const Color(0x1AFFFFFF),
          borderRadius: BorderRadius.circular(16.0),
        ),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: _withDividers(children),
        ),
      ),
    );
  }

  List<Widget> _withDividers(List<Widget> widgets) {
    if (widgets.isEmpty) return [];
    final List<Widget> result = [];
    for (int i = 0; i < widgets.length; i++) {
      result.add(widgets[i]);
      if (i != widgets.length - 1) {
        result.add(Container(height: 0.5, color: const Color(0xFF262626)));
      }
    }
    return result;
  }
}
