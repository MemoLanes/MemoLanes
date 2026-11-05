import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:material_ui/material_ui.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:memolanes/body/map/screenshot/screenshot_caption.dart';
import 'package:memolanes/src/rust/journey_header.dart';

enum ScreenshotLogoPosition { topRight, bottomRight, hidden }

/// One composition for both the scaled preview and the full-resolution export.
class ScreenshotComposition extends StatelessWidget {
  const ScreenshotComposition({
    super.key,
    required this.map,
    required this.logicalSize,
    required this.includeInfo,
    required this.logoPosition,
    this.timeRange,
    this.journey,
    this.logo,
  });

  final ui.Image map;
  final Size logicalSize;
  final bool includeInfo;
  final DateTimeRange? timeRange;
  final ScreenshotLogoPosition logoPosition;
  final JourneyHeader? journey;
  final PictureInfo? logo;

  static const _imageEdgeInset = 20.0;
  static const _logoMaxHeight = 32.0;
  static const _captionLogoGap = 12.0;

  @override
  Widget build(BuildContext context) {
    final picture = logo;
    final journey = this.journey;
    final showLogo =
        picture != null && logoPosition != ScreenshotLogoPosition.hidden;
    final logoWidth = math.min(
      math.min(96.0, _logoMaxHeight * (picture?.size.aspectRatio ?? 1)),
      math.max(0.0, logicalSize.width - _imageEdgeInset * 2),
    );
    final bottomLogo =
        showLogo && logoPosition == ScreenshotLogoPosition.bottomRight;
    final watermark = showLogo
        ? SizedBox(
            width: logoWidth,
            child: AspectRatio(
              aspectRatio: picture.size.aspectRatio,
              child: CustomPaint(painter: _ScreenshotLogoPainter(picture)),
            ),
          )
        : null;
    return SizedBox.fromSize(
      size: logicalSize,
      child: ClipRect(
        child: Stack(
          fit: StackFit.expand,
          children: [
            // Content switches only affect overlays, never the frozen map.
            RawImage(image: map, fit: BoxFit.fill),
            if (includeInfo && (journey != null || timeRange != null))
              Positioned(
                left: 0,
                right: 0,
                bottom: 0,
                child: Container(
                  constraints: BoxConstraints(
                    maxHeight: logicalSize.height * 0.55,
                  ),
                  decoration: const BoxDecoration(
                    gradient: LinearGradient(
                      begin: Alignment.topCenter,
                      end: Alignment.bottomCenter,
                      colors: [Colors.transparent, Color(0xb31a2925)],
                    ),
                  ),
                  padding: const EdgeInsets.fromLTRB(
                    _imageEdgeInset,
                    32,
                    _imageEdgeInset,
                    // Keep a fixed footer for the watermark, including when
                    // hidden or moved, so larger text never overlaps or jumps.
                    _imageEdgeInset + _logoMaxHeight + _captionLogoGap,
                  ),
                  child: FittedBox(
                    fit: BoxFit.scaleDown,
                    alignment: Alignment.bottomLeft,
                    child: SizedBox(
                      width: math.max(
                        0,
                        logicalSize.width - _imageEdgeInset * 2,
                      ),
                      child: ScreenshotCaption(
                        journey: journey,
                        timeRange: timeRange,
                      ),
                    ),
                  ),
                ),
              ),
            if (watermark != null)
              Positioned(
                right: _imageEdgeInset,
                top: logoPosition == ScreenshotLogoPosition.topRight
                    ? _imageEdgeInset
                    : null,
                bottom: bottomLogo ? _imageEdgeInset : null,
                child: watermark,
              ),
          ],
        ),
      ),
    );
  }
}

class _ScreenshotLogoPainter extends CustomPainter {
  const _ScreenshotLogoPainter(this.logo);

  final PictureInfo logo;

  @override
  void paint(Canvas canvas, Size size) {
    // Retain vector paths until the composition is captured at export resolution.
    canvas.save();
    canvas.scale(size.width / logo.size.width);
    canvas.drawPicture(logo.picture);
    canvas.restore();
  }

  @override
  bool shouldRepaint(_ScreenshotLogoPainter oldDelegate) =>
      logo != oldDelegate.logo;
}
