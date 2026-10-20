import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/journey/compact_journey_info_card.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:share_plus/share_plus.dart';

const screenshotLogoAsset = 'assets/screenshot_logo.png';
const _galleryChannel = MethodChannel('com.memolanes/screenshots');

Future<ui.Image> decodeScreenshot(Uint8List bytes) async {
  final codec = await ui.instantiateImageCodec(bytes);
  try {
    return (await codec.getNextFrame()).image;
  } finally {
    codec.dispose();
  }
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
    this.centeredJourneyMap,
  });

  final ui.Image map;
  final Size logicalSize;
  final ui.Image? timeline;
  final JourneyHeader? journey;
  final ui.Image? centeredJourneyMap;

  @override
  State<ScreenshotPreviewPage> createState() => _ScreenshotPreviewPageState();
}

class _ScreenshotPreviewPageState extends State<ScreenshotPreviewPage> {
  static const _imageEdgeInset = 20.0;

  final _compositionKey = GlobalKey();
  final _shareButtonKey = GlobalKey();
  ui.Image? _logo;
  bool _loadingLogo = true;
  bool _includeLogo = true;
  bool _includeJourneyInfo = true;
  bool _saving = false;
  bool _sharing = false;
  String? _error;

  bool get _busy => _saving || _sharing;

  bool get _logoAtTopRight => widget.timeline != null || widget.journey != null;

  ui.Image get _mapImage => !_includeJourneyInfo && widget.journey != null
      ? widget.centeredJourneyMap ?? widget.map
      : widget.map;

  @override
  void initState() {
    super.initState();
    _loadLogo();
  }

  Future<void> _loadLogo() async {
    ui.Image? logo;
    try {
      final manifest = await AssetManifest.loadFromAssetBundle(rootBundle);
      if (manifest.listAssets().contains(screenshotLogoAsset)) {
        final data = await rootBundle.load(screenshotLogoAsset);
        logo = await decodeScreenshot(
          data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes),
        );
      }
    } catch (error, stackTrace) {
      log.error('Loading screenshot logo failed: $error', stackTrace);
    }
    if (!mounted) {
      logo?.dispose();
      return;
    }
    setState(() {
      _logo = logo;
      _loadingLogo = false;
      if (logo == null) _includeLogo = false;
    });
  }

  @override
  void dispose() {
    _logo?.dispose();
    super.dispose();
  }

  Future<Uint8List> _encodePreview() async {
    final image = await captureScreenshotBoundary(
      _compositionKey,
      _mapImage.width / widget.logicalSize.width,
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
              aspectRatio: _logo!.width / _logo!.height,
              child: RawImage(image: _logo, fit: BoxFit.contain),
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
              RawImage(image: _mapImage, fit: BoxFit.fill),
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

  @override
  Widget build(BuildContext context) {
    return PopScope(
      canPop: !_busy,
      child: Scaffold(
        appBar: AppBar(
          title: Text(context.tr('screenshot.title')),
          automaticallyImplyLeading: false,
        ),
        body: PageContentFrame(
          padding: EdgeInsets.zero,
          child: SafeArea(
            child: Column(
              children: [
                Expanded(
                  child: Padding(
                    padding: const EdgeInsets.all(16),
                    child: Center(
                      child: FittedBox(
                        fit: BoxFit.contain,
                        child: IgnorePointer(child: _buildComposition()),
                      ),
                    ),
                  ),
                ),
                if (widget.journey != null)
                  SwitchListTile(
                    title: Text(context.tr('screenshot.include_journey_info')),
                    value: _includeJourneyInfo,
                    onChanged: _busy
                        ? null
                        : (value) =>
                              setState(() => _includeJourneyInfo = value),
                  ),
                SwitchListTile(
                  title: Text(
                    context.tr(
                      _logoAtTopRight
                          ? 'screenshot.include_logo_top_right'
                          : 'screenshot.include_logo',
                    ),
                  ),
                  subtitle: !_loadingLogo && _logo == null
                      ? Text(context.tr('screenshot.logo_unavailable'))
                      : null,
                  value: _includeLogo,
                  onChanged: _busy || _logo == null
                      ? null
                      : (value) {
                          setState(() => _includeLogo = value);
                        },
                ),
                if (_error != null)
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: Text(context.tr(_error!)),
                  ),
                Padding(
                  padding: const EdgeInsets.all(16),
                  child: IntrinsicHeight(
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.stretch,
                      children: [
                        Expanded(
                          child: AppButton(
                            label: context.tr('common.cancel'),
                            labelMaxLines: 3,
                            variant: AppButtonVariant.secondary,
                            onPressed: _busy
                                ? null
                                : () => Navigator.of(context).pop(),
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: AppButton(
                            label: context.tr('screenshot.save'),
                            labelMaxLines: 3,
                            loading: _saving,
                            onPressed: _busy || _loadingLogo ? null : _save,
                          ),
                        ),
                        const SizedBox(width: 8),
                        Expanded(
                          child: AppButton(
                            key: _shareButtonKey,
                            label: context.tr('common.share'),
                            labelMaxLines: 3,
                            variant: AppButtonVariant.secondary,
                            loading: _sharing,
                            onPressed: _busy || _loadingLogo ? null : _share,
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
