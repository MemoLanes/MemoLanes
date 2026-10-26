import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:share_plus/share_plus.dart';

const _galleryChannel = MethodChannel('com.memolanes/screenshots');

Future<ui.Image> decodeScreenshot(Uint8List bytes) async {
  final codec = await ui.instantiateImageCodec(bytes);
  try {
    return (await codec.getNextFrame()).image;
  } finally {
    codec.dispose();
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

/// Captures Flutter content only. Never wrap the platform WebView in this.
Future<ui.Image> captureScreenshotBoundary(
  GlobalKey key,
  double pixelRatio,
) async {
  await WidgetsBinding.instance.endOfFrame;
  final boundary = key.currentContext?.findRenderObject();
  if (boundary is! RenderRepaintBoundary || !boundary.hasSize) {
    throw StateError('Screenshot content is unavailable');
  }
  return boundary.toImage(pixelRatio: pixelRatio);
}

/// The caller owns the frozen images and disposes them when this route closes.
class ScreenshotPreviewPage extends StatefulWidget {
  const ScreenshotPreviewPage({
    super.key,
    required this.map,
    required this.logicalSize,
    this.timeline,
    this.journey,
  });

  final ui.Image map;
  final Size logicalSize;
  final ui.Image? timeline;
  final JourneyHeader? journey;

  @override
  State<ScreenshotPreviewPage> createState() => _ScreenshotPreviewPageState();
}

class _ScreenshotPreviewPageState extends State<ScreenshotPreviewPage> {
  static const _imageEdgeInset = 20.0;

  final _compositionKey = GlobalKey();
  final _shareButtonKey = GlobalKey();
  PictureInfo? _logo;
  String? _logoAsset;
  int _logoLoadGeneration = 0;
  bool _loadingLogo = true;
  bool _includeLogo = true;
  bool _includeJourneyInfo = true;
  bool _saving = false;
  bool _sharing = false;
  String? _error;

  bool get _busy => _saving || _sharing;

  bool get _logoAtTopRight => widget.timeline != null || widget.journey != null;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final asset = context.locale.languageCode == 'zh'
        ? 'assets/screenshot_logo_zh.svg'
        : 'assets/screenshot_logo_en.svg';
    if (_logoAsset == asset) return;
    _logoAsset = asset;
    _loadingLogo = true;
    _loadLogo(asset);
  }

  Future<void> _loadLogo(String asset) async {
    final generation = ++_logoLoadGeneration;
    PictureInfo? logo;
    try {
      logo = await vg.loadPicture(SvgAssetLoader(asset), null);
    } catch (error, stackTrace) {
      log.error('Loading screenshot logo failed: $error', stackTrace);
    }
    if (!mounted || generation != _logoLoadGeneration) {
      logo?.picture.dispose();
      return;
    }
    final previousLogo = _logo;
    setState(() {
      _logo = logo;
      _loadingLogo = false;
      if (logo == null) _includeLogo = false;
    });
    previousLogo?.picture.dispose();
  }

  @override
  void dispose() {
    _logo?.picture.dispose();
    super.dispose();
  }

  Future<Uint8List> _encodePreview() async {
    final image = await captureScreenshotBoundary(
      _compositionKey,
      widget.map.width / widget.logicalSize.width,
    );
    try {
      final data = await image.toByteData(format: ui.ImageByteFormat.png);
      if (data == null) throw StateError('PNG encoding failed');
      return data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes);
    } finally {
      image.dispose();
    }
  }

  Future<void> _share() async {
    if (_busy || _loadingLogo) return;
    setState(() {
      _sharing = true;
      _error = null;
    });
    try {
      final png = await _encodePreview();
      if (!mounted) return;
      // iPad needs a valid popover anchor, measured after encoding in case
      // the device rotated while the image was being prepared.
      final box = _shareButtonKey.currentContext?.findRenderObject();
      if (box is! RenderBox || !box.hasSize || box.size.isEmpty) {
        throw StateError('Share button is unavailable');
      }
      await SharePlus.instance.share(
        ShareParams(
          files: [XFile.fromData(png, mimeType: 'image/png')],
          fileNameOverrides: [
            'MemoLanes_${DateTime.now().millisecondsSinceEpoch}.png',
          ],
          sharePositionOrigin: box.localToGlobal(Offset.zero) & box.size,
        ),
      );
      // Keep the preview open after sharing or dismissing the system sheet.
    } catch (error, stackTrace) {
      log.error('Sharing screenshot failed: $error', stackTrace);
      if (mounted) setState(() => _error = 'screenshot.share_failed');
    } finally {
      if (mounted) setState(() => _sharing = false);
    }
  }

  Future<void> _save() async {
    if (_busy || _loadingLogo) return;
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final png = await _encodePreview();
      await _galleryChannel.invokeMethod<void>('saveToPhotos', {'png': png});
      if (mounted) Navigator.of(context).pop(true);
    } catch (error, stackTrace) {
      log.error('Saving screenshot failed: $error', stackTrace);
      if (mounted) {
        setState(
          () => _error =
              error is PlatformException && error.code == 'permission_denied'
              ? 'screenshot.permission_denied'
              : 'screenshot.save_failed',
        );
      }
    } finally {
      if (mounted) setState(() => _saving = false);
    }
  }

  Widget _buildComposition() {
    final logo = _includeLogo && _logo != null
        ? SizedBox(
            width: 120,
            child: AspectRatio(
              aspectRatio: _logo!.size.aspectRatio,
              child: CustomPaint(painter: _ScreenshotLogoPainter(_logo!)),
            ),
          )
        : null;
    final timeline = widget.timeline;
    final journey = widget.journey;
    return RepaintBoundary(
      key: _compositionKey,
      child: SizedBox.fromSize(
        size: widget.logicalSize,
        child: ClipRect(
          child: Stack(
            fit: StackFit.expand,
            children: [
              // Content switches only affect overlays, never the frozen map.
              RawImage(image: widget.map, fit: BoxFit.fill),
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
              if (journey != null && _includeJourneyInfo)
                Positioned(
                  left: _imageEdgeInset,
                  right: _imageEdgeInset,
                  bottom: _imageEdgeInset,
                  child: Align(
                    alignment: Alignment.bottomRight,
                    child: ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: 430,
                        maxHeight: widget.logicalSize.height * 0.65,
                      ),
                      child: FittedBox(
                        fit: BoxFit.scaleDown,
                        alignment: Alignment.bottomRight,
                        child: SizedBox(
                          width:
                              (widget.logicalSize.width - _imageEdgeInset * 2)
                                  .clamp(0, 430),
                          child: ReadOnlyJourneyInfoCard(journey: journey),
                        ),
                      ),
                    ),
                  ),
                ),
              if (logo != null)
                Positioned(
                  right: _imageEdgeInset,
                  top: _logoAtTopRight ? _imageEdgeInset : null,
                  bottom: _logoAtTopRight ? null : _imageEdgeInset,
                  child: logo,
                ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildPreview() => LayoutBuilder(
    builder: (context, constraints) {
      final colors = context.appColors;
      final scale = math.min(
        constraints.maxWidth / widget.logicalSize.width,
        constraints.maxHeight / widget.logicalSize.height,
      );
      if (scale <= 0) return const SizedBox.shrink();
      const radius = BorderRadius.all(Radius.circular(18));
      return Center(
        child: DecoratedBox(
          decoration: BoxDecoration(
            borderRadius: radius,
            boxShadow: [
              BoxShadow(
                color: colors.shadowColor.withValues(alpha: 0.1),
                blurRadius: 28,
                offset: const Offset(0, 8),
              ),
            ],
          ),
          child: SizedBox.fromSize(
            size: widget.logicalSize * scale,
            // Preview decoration stays outside the exported composition.
            child: ClipRRect(
              borderRadius: radius,
              child: FittedBox(
                fit: BoxFit.contain,
                child: IgnorePointer(child: _buildComposition()),
              ),
            ),
          ),
        ),
      );
    },
  );

  Widget _buildOption({
    required IconData icon,
    required String label,
    required String tooltip,
    required bool selected,
    required ValueChanged<bool>? onSelected,
  }) {
    final colors = context.appColors;
    final foreground = onSelected == null
        ? colors.subtleInkColor
        : selected
        ? colors.deepGreen
        : colors.mutedInkColor;
    return FilterChip(
      avatar: Icon(icon, size: 19, color: foreground),
      label: Row(
        children: [
          Expanded(
            child: Text(label, maxLines: 2, overflow: TextOverflow.ellipsis),
          ),
          const SizedBox(width: 8),
          Icon(
            selected ? Icons.check_circle_rounded : Icons.circle_outlined,
            size: 18,
            color: foreground,
          ),
        ],
      ),
      labelStyle: AppTypography.supporting.copyWith(
        color: foreground,
        fontWeight: FontWeight.w600,
      ),
      labelPadding: const EdgeInsets.only(left: 8),
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      side: BorderSide(
        color: selected
            ? colors.deepGreen.withValues(alpha: 0.38)
            : colors.strongLineColor,
      ),
      color: WidgetStatePropertyAll(
        selected ? colors.selectedSurfaceColor : colors.surfaceColor,
      ),
      showCheckmark: false,
      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
      selected: selected,
      onSelected: onSelected,
      tooltip: tooltip,
    );
  }

  Widget _buildTools() {
    final colors = context.appColors;
    final optionHeight = math.max(
      48.0,
      MediaQuery.textScalerOf(context).scale(13) * 2 * 1.4 + 20,
    );
    final logoOption = _buildOption(
      icon: Icons.image_outlined,
      label: context.tr('screenshot.logo_option'),
      tooltip: context.tr(
        _logoAtTopRight
            ? 'screenshot.include_logo_top_right'
            : 'screenshot.include_logo',
      ),
      selected: _includeLogo,
      onSelected: _busy || _loadingLogo || _logo == null
          ? null
          : (value) => setState(() => _includeLogo = value),
    );
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 12, 20, 16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Divider(height: 1, color: colors.lineColor),
          const SizedBox(height: 12),
          Text(
            context.tr('screenshot.content_options'),
            style: AppTypography.caption.copyWith(color: colors.mutedInkColor),
          ),
          const SizedBox(height: 10),
          SizedBox(
            height: optionHeight,
            child: widget.journey == null
                ? ConstrainedBox(
                    constraints: const BoxConstraints(maxWidth: 200),
                    child: logoOption,
                  )
                : Row(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Expanded(
                        child: _buildOption(
                          icon: Icons.route_rounded,
                          label: context.tr('screenshot.journey_info_option'),
                          tooltip: context.tr(
                            'screenshot.include_journey_info',
                          ),
                          selected: _includeJourneyInfo,
                          onSelected: _busy
                              ? null
                              : (value) =>
                                    setState(() => _includeJourneyInfo = value),
                        ),
                      ),
                      const SizedBox(width: 10),
                      Expanded(child: logoOption),
                    ],
                  ),
          ),
          if (!_loadingLogo && _logo == null) ...[
            const SizedBox(height: 10),
            Text(
              context.tr('screenshot.logo_unavailable'),
              style: AppTypography.caption.copyWith(
                color: colors.mutedInkColor,
              ),
            ),
          ],
          if (_error != null) ...[
            const SizedBox(height: 12),
            Semantics(
              liveRegion: true,
              child: Text(
                context.tr(_error!),
                style: AppTypography.supporting.copyWith(
                  color: colors.dangerInkColor,
                ),
              ),
            ),
          ],
          const SizedBox(height: 14),
          IntrinsicHeight(
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Expanded(
                  flex: 3,
                  child: AppButton(
                    label: context.tr('screenshot.save'),
                    icon: Icons.save_alt_rounded,
                    size: AppButtonSize.large,
                    borderRadius: 18,
                    labelMaxLines: 2,
                    loading: _saving,
                    onPressed: _busy || _loadingLogo ? null : _save,
                  ),
                ),
                const SizedBox(width: 10),
                Expanded(
                  flex: 2,
                  child: AppButton(
                    key: _shareButtonKey,
                    label: context.tr('common.share'),
                    icon: Icons.ios_share_rounded,
                    size: AppButtonSize.large,
                    borderRadius: 18,
                    labelMaxLines: 2,
                    variant: AppButtonVariant.secondary,
                    loading: _sharing,
                    onPressed: _busy || _loadingLogo ? null : _share,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final colors = context.appColors;
    return PopScope(
      canPop: !_busy,
      child: Scaffold(
        appBar: AppBar(
          title: Text(context.tr('screenshot.title')),
          titleTextStyle: AppTypography.surfaceTitle.copyWith(
            color: colors.inkColor,
            fontWeight: FontWeight.w600,
          ),
          centerTitle: true,
          automaticallyImplyLeading: false,
          leadingWidth: 72,
          leading: Padding(
            padding: const EdgeInsets.only(left: 16),
            child: Center(
              child: IconButton.outlined(
                tooltip: MaterialLocalizations.of(context).closeButtonTooltip,
                onPressed: _busy ? null : () => Navigator.of(context).pop(),
                style: IconButton.styleFrom(
                  foregroundColor: colors.inkColor,
                  backgroundColor: colors.surfaceColor,
                  side: BorderSide(color: colors.lineColor),
                  fixedSize: const Size.square(44),
                ),
                icon: const Icon(Icons.close_rounded, size: 21),
              ),
            ),
          ),
        ),
        body: PageContentInsets(
          maintainViewPadding: true,
          child: SafeArea(
            top: false,
            child: LayoutBuilder(
              builder: (context, constraints) {
                final landscape =
                    constraints.maxWidth >= 600 &&
                    constraints.maxWidth > constraints.maxHeight;
                final preview = Padding(
                  padding: const EdgeInsets.fromLTRB(20, 8, 20, 12),
                  child: _buildPreview(),
                );
                if (landscape) {
                  return Row(
                    children: [
                      Expanded(child: preview),
                      SizedBox(
                        width: math.min(340.0, constraints.maxWidth * 0.42),
                        child: Center(
                          child: SingleChildScrollView(
                            primary: false,
                            child: _buildTools(),
                          ),
                        ),
                      ),
                    ],
                  );
                }
                return Column(
                  children: [
                    Expanded(child: preview),
                    ConstrainedBox(
                      constraints: BoxConstraints(
                        maxWidth: PageContentFrame.maxContentWidth,
                        maxHeight: constraints.maxHeight * 0.55,
                      ),
                      child: SingleChildScrollView(
                        primary: false,
                        child: _buildTools(),
                      ),
                    ),
                  ],
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}
