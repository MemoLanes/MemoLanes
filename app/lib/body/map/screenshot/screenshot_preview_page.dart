import 'dart:math' as math;
import 'dart:ui' as ui;

import 'package:easy_localization/easy_localization.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_svg/flutter_svg.dart';
import 'package:material_ui/material_ui.dart';
import 'package:memolanes/body/map/screenshot/screenshot_composition.dart';
import 'package:memolanes/common/component/app_button.dart';
import 'package:memolanes/common/component/page_content_layout.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/common/screenshot.dart';
import 'package:memolanes/constants/app_typography.dart';
import 'package:memolanes/src/rust/journey_header.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:share_plus/share_plus.dart';

/// The caller owns the frozen images and disposes them when this route closes.
class ScreenshotPreviewPage extends StatefulWidget {
  const ScreenshotPreviewPage({
    super.key,
    required this.map,
    required this.logicalSize,
    this.timeRange,
    this.journey,
  });

  final ui.Image map;
  final Size logicalSize;
  final DateTimeRange? timeRange;
  final JourneyHeader? journey;

  @override
  State<ScreenshotPreviewPage> createState() => _ScreenshotPreviewPageState();
}

enum _ExportAction { save, share }

class _ScreenshotPreviewPageState extends State<ScreenshotPreviewPage> {
  final _compositionKey = GlobalKey();
  final _shareButtonKey = GlobalKey();
  PictureInfo? _logo;
  String? _logoAsset;
  Future<PictureInfo>? _logoRequest;
  ScreenshotLogoPosition _logoPosition = ScreenshotLogoPosition.bottomRight;
  bool _includeInfo = true;
  _ExportAction? _exporting;
  String? _error;

  bool get _busy => _exporting != null;
  bool get _loadingLogo => _logoRequest != null;

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    final asset = context.locale.languageCode == 'zh'
        ? 'assets/screenshot_logo_zh.svg'
        : 'assets/screenshot_logo_en.svg';
    if (_logoAsset == asset) return;
    _logoAsset = asset;
    _loadLogo(asset);
  }

  Future<void> _loadLogo(String asset) async {
    final request = vg.loadPicture(SvgAssetLoader(asset), null);
    _logoRequest = request;
    PictureInfo? logo;
    try {
      logo = await request;
    } catch (error, stackTrace) {
      log.error(
        'Loading screenshot watermark $asset failed: $error',
        stackTrace,
      );
    }
    if (!mounted || request != _logoRequest) {
      logo?.picture.dispose();
      return;
    }
    final previousLogo = _logo;
    setState(() {
      _logo = logo;
      _logoRequest = null;
      if (logo == null) _logoPosition = ScreenshotLogoPosition.hidden;
    });
    previousLogo?.picture.dispose();
  }

  @override
  void dispose() {
    _logo?.picture.dispose();
    super.dispose();
  }

  Future<Uint8List> _encodePreview() async {
    await WidgetsBinding.instance.endOfFrame;
    final boundary = _compositionKey.currentContext?.findRenderObject();
    if (boundary is! RenderRepaintBoundary || !boundary.hasSize) {
      throw StateError('Screenshot content is unavailable');
    }
    final image = await boundary.toImage(
      pixelRatio: widget.map.width / widget.logicalSize.width,
    );
    try {
      final data = await image.toByteData(format: ui.ImageByteFormat.png);
      if (data == null) throw StateError('PNG encoding failed');
      return data.buffer.asUint8List(data.offsetInBytes, data.lengthInBytes);
    } finally {
      image.dispose();
    }
  }

  Future<void> _sharePng(Uint8List png) async {
    // iPad needs a popover anchor in the current layout.
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
  }

  Future<void> _export(_ExportAction action) async {
    if (_busy || _loadingLogo) return;
    setState(() {
      _exporting = action;
      _error = null;
    });
    try {
      final png = await _encodePreview();
      if (!mounted) return;
      if (action == _ExportAction.save) {
        await screenshotChannel.invokeMethod<void>('saveToPhotos', {
          'png': png,
        });
        if (mounted) Navigator.of(context).pop(true);
      } else {
        await _sharePng(png);
      }
    } catch (error, stackTrace) {
      log.error('Screenshot ${action.name} failed: $error', stackTrace);
      if (mounted) {
        setState(
          () => _error = switch (action) {
            _ExportAction.save
                when error is PlatformException &&
                    error.code == 'permission_denied' =>
              'screenshot.permission_denied',
            _ExportAction.save => 'screenshot.save_failed',
            _ExportAction.share => 'screenshot.share_failed',
          },
        );
      }
    } finally {
      if (mounted) setState(() => _exporting = null);
    }
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
                child: IgnorePointer(
                  child: RepaintBoundary(
                    key: _compositionKey,
                    child: ScreenshotComposition(
                      map: widget.map,
                      logicalSize: widget.logicalSize,
                      timeRange: widget.timeRange,
                      journey: widget.journey,
                      includeInfo: _includeInfo,
                      logo: _logo,
                      logoPosition: _logoPosition,
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
    },
  );

  Widget _buildInfoOption() {
    final colors = context.appColors;
    final isJourney = widget.journey != null;
    final foreground = _busy
        ? colors.subtleInkColor
        : _includeInfo
        ? colors.deepGreen
        : colors.mutedInkColor;
    return FilterChip(
      avatar: Icon(
        isJourney ? Icons.route_rounded : Icons.date_range_rounded,
        size: 19,
        color: foreground,
      ),
      label: Row(
        children: [
          Expanded(
            child: Text(
              context.tr(
                isJourney
                    ? 'screenshot.journey_info_option'
                    : 'screenshot.time_info_option',
              ),
              maxLines: 2,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 8),
          Icon(
            _includeInfo ? Icons.check_circle_rounded : Icons.circle_outlined,
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
        color: _includeInfo
            ? colors.deepGreen.withValues(alpha: 0.38)
            : colors.strongLineColor,
      ),
      color: WidgetStatePropertyAll(
        _includeInfo ? colors.selectedSurfaceColor : colors.surfaceColor,
      ),
      showCheckmark: false,
      materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
      selected: _includeInfo,
      onSelected: _busy
          ? null
          : (value) => setState(() => _includeInfo = value),
      tooltip: context.tr(
        isJourney
            ? 'screenshot.include_journey_info'
            : 'screenshot.include_time_info',
      ),
    );
  }

  Widget _buildTools() {
    final colors = context.appColors;
    final canExport = !_busy && !_loadingLogo;
    final optionHeight = math.max(
      48.0,
      MediaQuery.textScalerOf(context).scale(13) * 2 * 1.4 + 20,
    );
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 12, 20, 16),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Divider(height: 1, color: colors.lineColor),
          const SizedBox(height: 12),
          if (widget.journey != null || widget.timeRange != null) ...[
            Text(
              context.tr('screenshot.content_options'),
              style: AppTypography.caption.copyWith(
                color: colors.mutedInkColor,
              ),
            ),
            const SizedBox(height: 10),
            SizedBox(height: optionHeight, child: _buildInfoOption()),
            const SizedBox(height: 10),
          ],
          Text(
            context.tr('screenshot.logo_option'),
            style: AppTypography.caption.copyWith(color: colors.mutedInkColor),
          ),
          const SizedBox(height: 4),
          SegmentedButton<ScreenshotLogoPosition>(
            expandedInsets: EdgeInsets.zero,
            showSelectedIcon: false,
            segments: [
              for (final position in ScreenshotLogoPosition.values)
                ButtonSegment(
                  value: position,
                  label: Text(
                    context.tr('screenshot.logo_${position.name}'),
                    textAlign: TextAlign.center,
                    maxLines: 2,
                    overflow: TextOverflow.ellipsis,
                  ),
                ),
            ],
            selected: {_logoPosition},
            onSelectionChanged: !canExport || _logo == null
                ? null
                : (selection) =>
                      setState(() => _logoPosition = selection.single),
            style: ButtonStyle(
              backgroundColor: WidgetStateProperty.resolveWith((states) {
                return states.contains(WidgetState.selected)
                    ? colors.softGreen
                    : colors.surfaceColor;
              }),
              foregroundColor: WidgetStateProperty.resolveWith((states) {
                if (states.contains(WidgetState.disabled)) {
                  return colors.subtleInkColor;
                }
                return states.contains(WidgetState.selected)
                    ? colors.deepGreen
                    : colors.mutedInkColor;
              }),
              side: WidgetStatePropertyAll(BorderSide(color: colors.lineColor)),
              shape: WidgetStatePropertyAll(
                RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
              ),
              minimumSize: const WidgetStatePropertyAll(Size(0, 48)),
              padding: const WidgetStatePropertyAll(
                EdgeInsets.symmetric(horizontal: 8, vertical: 10),
              ),
              textStyle: const WidgetStatePropertyAll(AppTypography.label),
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
                    loading: _exporting == _ExportAction.save,
                    onPressed: canExport
                        ? () => _export(_ExportAction.save)
                        : null,
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
                    loading: _exporting == _ExportAction.share,
                    onPressed: canExport
                        ? () => _export(_ExportAction.share)
                        : null,
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
          toolbarHeight: 48,
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
                  padding: const EdgeInsets.fromLTRB(20, 4, 20, 8),
                  child: _buildPreview(),
                );
                final tools = SingleChildScrollView(
                  primary: false,
                  child: _buildTools(),
                );
                if (landscape) {
                  return Row(
                    children: [
                      Expanded(child: preview),
                      SizedBox(
                        width: math.min(340.0, constraints.maxWidth * 0.42),
                        child: Center(child: tools),
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
                      child: tools,
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
