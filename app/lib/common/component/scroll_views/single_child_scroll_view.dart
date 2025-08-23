import 'package:flutter/material.dart';

class MlSingleChildScrollView extends StatelessWidget {
  const MlSingleChildScrollView({
    super.key,
    required this.children,
    this.crossAxisAlignment,
    this.mainAxisAlignment,
    this.verticalDirection,
    this.padding,
    this.textDirection,
    this.textBaseline,
  });

  final List<Widget> children;
  final CrossAxisAlignment? crossAxisAlignment;
  final MainAxisAlignment? mainAxisAlignment;
  final VerticalDirection? verticalDirection;
  final TextDirection? textDirection;
  final TextBaseline? textBaseline;
  final EdgeInsetsGeometry? padding;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(builder: (context, constraint) {
      final allChildren = <Widget>[const SizedBox(width: double.infinity)];
      allChildren.addAll(children);
      return SingleChildScrollView(
        physics: AlwaysScrollableScrollPhysics(),
        child: Padding(
          padding: padding ?? EdgeInsets.zero,
          child: ConstrainedBox(
            constraints: BoxConstraints(
              minHeight: constraint.maxHeight - (padding?.vertical ?? 0),
            ),
            child: IntrinsicHeight(
              child: Column(
                crossAxisAlignment:
                    crossAxisAlignment ?? CrossAxisAlignment.center,
                mainAxisAlignment: mainAxisAlignment ?? MainAxisAlignment.start,
                mainAxisSize: MainAxisSize.max,
                verticalDirection: verticalDirection ?? VerticalDirection.down,
                textBaseline: textBaseline,
                textDirection: textDirection,
                children: allChildren,
              ),
            ),
          ),
        ),
      );
    });
  }
}
