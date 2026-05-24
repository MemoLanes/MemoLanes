import 'package:flutter/material.dart';

class TabContentTransition extends StatelessWidget {
  const TabContentTransition({
    super.key,
    required this.transitionKey,
    required this.child,
    this.duration = const Duration(milliseconds: 260),
    this.reverseDuration = const Duration(milliseconds: 180),
    this.curve = Curves.easeOutCubic,
    this.offset = const Offset(0, 0.025),
  });

  final LocalKey transitionKey;
  final Widget child;
  final Duration duration;
  final Duration reverseDuration;
  final Curve curve;
  final Offset offset;

  @override
  Widget build(BuildContext context) {
    return AnimatedSwitcher(
      duration: duration,
      reverseDuration: reverseDuration,
      switchInCurve: curve,
      switchOutCurve: curve,
      layoutBuilder: (currentChild, previousChildren) {
        return Stack(
          fit: StackFit.expand,
          children: [
            ...previousChildren,
            if (currentChild != null) currentChild,
          ],
        );
      },
      transitionBuilder: (child, animation) {
        final curvedAnimation = animation.drive(CurveTween(curve: curve));
        final slideAnimation = Tween<Offset>(
          begin: offset,
          end: Offset.zero,
        ).animate(curvedAnimation);

        return AnimatedBuilder(
          animation: animation,
          child: FadeTransition(
            opacity: curvedAnimation,
            child: SlideTransition(
              position: slideAnimation,
              child: child,
            ),
          ),
          builder: (context, child) {
            return IgnorePointer(
              ignoring: animation.status == AnimationStatus.reverse,
              child: child,
            );
          },
        );
      },
      child: KeyedSubtree(
        key: transitionKey,
        child: child,
      ),
    );
  }
}
