import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart'
    show formatJourneyInfoTime;
import 'package:memolanes/common/journey_kind_visuals.dart';
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
    final logoAspectRatio = picture?.size.aspectRatio ?? 1;
    final logoWidth = math.min(
      math.min(96.0, _logoMaxHeight * logoAspectRatio),
      math.max(0.0, logicalSize.width - _imageEdgeInset * 2),
    );
    return SizedBox.fromSize(
      size: logicalSize,
      child: ClipRect(
        child: Stack(
          fit: StackFit.expand,
          children: [
            // The WebView uses black behind the transparent globe canvas.
            ColoredBox(
              color: Colors.black,
              child: RawImage(image: map, fit: BoxFit.fill),
            ),
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
                      child: _ScreenshotCaption(
                        journey: journey,
                        timeRange: timeRange,
                      ),
                    ),
                  ),
                ),
              ),
            if (picture != null &&
                logoPosition != ScreenshotLogoPosition.hidden)
              Positioned(
                right: _imageEdgeInset,
                top: logoPosition == ScreenshotLogoPosition.topRight
                    ? _imageEdgeInset
                    : null,
                bottom: logoPosition == ScreenshotLogoPosition.bottomRight
                    ? _imageEdgeInset
                    : null,
                child: CustomPaint(
                  size: Size(logoWidth, logoWidth / logoAspectRatio),
                  painter: _ScreenshotLogoPainter(picture),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _ScreenshotCaption extends StatelessWidget {
  const _ScreenshotCaption({this.journey, this.timeRange})
    : assert((journey != null) != (timeRange != null));

  final JourneyHeader? journey;
  final DateTimeRange? timeRange;

  @override
  Widget build(BuildContext context) {
    final journey = this.journey;
    final range = timeRange;
    final format = DateFormat('yyyy.MM.dd');
    final title = journey != null
        ? format.format(journey.journeyDate)
        : range!.start == range.end
        ? format.format(range.start)
        : '${format.format(range.start)} — ${format.format(range.end)}';
    final note = journey?.note?.trim();
    return DefaultTextStyle(
      style: const TextStyle(
        fontSize: 15,
        height: 1.5,
        color: Colors.white,
        shadows: [Shadow(color: Color(0x80000000), blurRadius: 6)],
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            journey != null
                ? journeyKindLabel(context, journey.journeyKind)
                : context.tr('screenshot.selected_time'),
            style: const TextStyle(
              fontSize: 14,
              color: Color(0xffc9e8cd),
              letterSpacing: 1,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            title,
            style: const TextStyle(fontSize: 24, fontWeight: FontWeight.w600),
          ),
          if (journey != null &&
              (journey.start != null || journey.end != null)) ...[
            const SizedBox(height: 6),
            Text(
              '${formatJourneyInfoTime(journey.start)} — ${formatJourneyInfoTime(journey.end)}',
            ),
          ],
          if (note != null && note.isNotEmpty) ...[
            const SizedBox(height: 6),
            Text(note, maxLines: 2, overflow: TextOverflow.ellipsis),
          ],
        ],
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
