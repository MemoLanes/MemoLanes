import 'dart:ui' as ui;

import 'package:flutter/widgets.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/src/rust/journey_header.dart';

/// One composition for both the scaled preview and the full-resolution export.
class ScreenshotComposition extends StatelessWidget {
  const ScreenshotComposition({
    super.key,
    required this.map,
    required this.logicalSize,
    required this.includeJourneyInfo,
    this.timeline,
    this.journey,
    this.logo,
  });

  final ui.Image map;
  final Size logicalSize;
  final bool includeJourneyInfo;
  final ui.Image? timeline;
  final JourneyHeader? journey;
  final PictureInfo? logo;

  static const _imageEdgeInset = 20.0;
  bool get _logoAtTopRight => timeline != null || journey != null;

  @override
  Widget build(BuildContext context) {
    final picture = logo;
    final timeline = this.timeline;
    final journey = this.journey;
    final watermark = picture != null
        ? SizedBox(
            width: 120,
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
            if (timeline != null)
              Positioned(
                left: _imageEdgeInset,
                right: _imageEdgeInset,
                bottom: _imageEdgeInset,
                child: AspectRatio(
                  aspectRatio: timeline.width / timeline.height,
                  child: RawImage(image: timeline, fit: BoxFit.contain),
                ),
              ),
            if (journey != null && includeJourneyInfo)
              Positioned(
                left: _imageEdgeInset,
                right: _imageEdgeInset,
                bottom: _imageEdgeInset,
                child: Align(
                  alignment: Alignment.bottomRight,
                  child: ConstrainedBox(
                    constraints: BoxConstraints(
                      maxWidth: 430,
                      maxHeight: logicalSize.height * 0.65,
                    ),
                    child: FittedBox(
                      fit: BoxFit.scaleDown,
                      alignment: Alignment.bottomRight,
                      child: SizedBox(
                        width: (logicalSize.width - _imageEdgeInset * 2).clamp(
                          0,
                          430,
                        ),
                        child: ReadOnlyJourneyInfoCard(journey: journey),
                      ),
                    ),
                  ),
                ),
              ),
            if (watermark != null)
              Positioned(
                right: _imageEdgeInset,
                top: _logoAtTopRight ? _imageEdgeInset : null,
                bottom: _logoAtTopRight ? null : _imageEdgeInset,
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
