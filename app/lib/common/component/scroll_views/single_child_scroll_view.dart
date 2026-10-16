import 'dart:math' as math;

import 'package:flutter/material.dart';

/// A viewport-filling column for non-flex form and page content.
class MlSingleChildScrollView extends StatelessWidget {
  const MlSingleChildScrollView({
    super.key,
    required this.children,
    this.mainAxisAlignment = MainAxisAlignment.start,
    this.padding,
  });

  final List<Widget> children;
  final MainAxisAlignment mainAxisAlignment;
  final EdgeInsetsGeometry? padding;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraint) {
        return SingleChildScrollView(
          physics: const AlwaysScrollableScrollPhysics(),
          padding: padding,
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minWidth: math.max(
                0,
                constraint.maxWidth - (padding?.horizontal ?? 0),
              ),
              minHeight: math.max(
                0,
                constraint.maxHeight - (padding?.vertical ?? 0),
              ),
            ),
            child: Column(
              mainAxisAlignment: mainAxisAlignment,
              children: children,
            ),
          ),
        );
      },
    );
  }
}
