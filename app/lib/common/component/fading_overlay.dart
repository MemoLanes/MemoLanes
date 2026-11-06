import 'package:material_ui/material_ui.dart';
import 'package:memolanes/common/app_layout.dart';

/// Fades one overlay in place, retaining its last layout until it is hidden.
class FadingOverlay extends StatefulWidget {
  const FadingOverlay({super.key, required this.child, this.animate = true});

  final Widget? child;
  final bool animate;

  @override
  State<FadingOverlay> createState() => _FadingOverlayState();
}

class _FadingOverlayState extends State<FadingOverlay> {
  Widget? _content;

  void _removeHiddenContent() {
    if (widget.child == null && _content != null) {
      setState(() => _content = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    final child = widget.child;
    final visible = child != null;
    final animate = widget.animate && !MediaQuery.disableAnimationsOf(context);
    if (child != null) {
      // Navigation geometry changes as soon as its visibility changes.
      _content = AppLayoutScope(
        layout: AppLayoutScope.of(context),
        child: Stack(fit: StackFit.expand, children: [child]),
      );
    } else if (!animate) {
      _content = null;
    }
    return Positioned.fill(
      child: IgnorePointer(
        ignoring: !visible,
        child: ExcludeFocus(
          excluding: !visible,
          child: ExcludeSemantics(
            excluding: !visible,
            child: AnimatedOpacity(
              opacity: visible ? 1 : 0,
              duration: animate
                  ? const Duration(milliseconds: 200)
                  : Duration.zero,
              curve: Curves.easeOutCubic,
              onEnd: _removeHiddenContent,
              child: _content,
            ),
          ),
        ),
      ),
    );
  }
}
