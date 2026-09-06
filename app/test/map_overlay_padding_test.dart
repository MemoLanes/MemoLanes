import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:memolanes/common/component/capsule_style_overlay_app_bar.dart';

void main() {
  for (final size in [const Size(640, 320), const Size(390, 844)]) {
    testWidgets('import preview leaves map space at $size', (tester) async {
      late EdgeInsets padding;
      const safeArea = EdgeInsets.only(top: 24, bottom: 24);
      await tester.pumpWidget(
        MediaQuery(
          data: MediaQueryData(
            size: size,
            padding: safeArea,
            viewPadding: safeArea,
          ),
          child: Builder(
            builder: (context) {
              padding = CapsuleStyleOverlayAppBar.mapFitPaddingForBottomOverlay(
                context,
                bottomOverlayHeight: safeArea.bottom + 246,
              );
              return const SizedBox.shrink();
            },
          ),
        ),
      );
      expect(size.height - padding.vertical, greaterThanOrEqualTo(120));
      expect(size.width - padding.horizontal, greaterThan(0));
      if (size.height > size.width) {
        expect(padding.bottom, safeArea.bottom + 270);
      }
    });
  }
}
