import 'package:flutter/material.dart';
import 'package:flutter_appbar/flutter_appbar.dart' as fappbar;
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

/// 胶囊式标题栏的「纯内容」区域：返回按钮 + 可选标题胶囊 + 可选更多按钮。
/// 不包含安全区或外层布局，由调用方决定放在 [Scaffold.appBar] 或 [AppBarConnection] 的 body 中。
class _CapsuleBarContent extends StatelessWidget {
  const _CapsuleBarContent({
    required this.showOnlyBackButton,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreMenuContent,
    this.moreIcon,
    this.foregroundColor,
    this.pillColor,
    this.subtitleFg,
  });

  final bool showOnlyBackButton;
  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  /// 若提供，则「更多」按钮用 [CustomPopup] 包裹，点击后弹出此内容（自动处理位置）。
  final Widget? moreMenuContent;
  final Widget? moreIcon;
  final Color? foregroundColor;
  final Color? pillColor;
  final Color? subtitleFg;

  static const double _barContentHeight = 44.0;
  static const double _pillRadius = 18.0;
  static const double _iconButtonSize = 36.0;
  static const Color _defaultFg = Color(0xFFE5E5E7);
  static const Color _defaultPill = Color(0xFF2C2C2E);
  static const Color _defaultSubtitleFg = Color(0xFF8E8E93);

  Color get _fg => foregroundColor ?? _defaultFg;
  Color get _pill => pillColor ?? _defaultPill;
  Color get _subFg => subtitleFg ?? _defaultSubtitleFg;

  Widget _pillButton(Widget icon, VoidCallback? onPressed) {
    return Material(
      color: _pill,
      shape: const CircleBorder(),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onPressed,
        customBorder: const CircleBorder(),
        child: SizedBox(
          width: _iconButtonSize,
          height: _iconButtonSize,
          child: Center(
            child: IconTheme.merge(
              data: IconThemeData(color: _fg, size: 20),
              child: icon,
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final onBackCallback = onBack ?? () => Navigator.maybePop(context);
    return SizedBox(
      height: _barContentHeight,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8.0),
        child: Row(
          children: [
            _pillButton(
              const Icon(Icons.arrow_back_ios_new, size: 20),
              onBackCallback,
            ),
            if (!showOnlyBackButton) ...[
              const SizedBox(width: 12),
              Expanded(
                child: Center(
                  child: Container(
                    padding: const EdgeInsets.symmetric(
                      horizontal: 14,
                      vertical: 5,
                    ),
                    decoration: BoxDecoration(
                      color: _pill,
                      borderRadius: BorderRadius.circular(_pillRadius),
                    ),
                    child: Column(
                      mainAxisSize: MainAxisSize.min,
                      mainAxisAlignment: MainAxisAlignment.center,
                      children: [
                        if (title != null && title!.isNotEmpty)
                          Text(
                            title!,
                            style: TextStyle(
                              color: _fg,
                              fontSize: 16,
                              fontWeight: FontWeight.w600,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            textAlign: TextAlign.center,
                          ),
                        if (subtitle != null && subtitle!.isNotEmpty) ...[
                          const SizedBox(height: 1),
                          Text(
                            subtitle!,
                            style: TextStyle(
                              color: _subFg,
                              fontSize: 11,
                            ),
                            maxLines: 1,
                            overflow: TextOverflow.ellipsis,
                            textAlign: TextAlign.center,
                          ),
                        ],
                      ],
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 12),
              if (moreMenuContent != null)
                CustomPopup(
                  position: PopupPosition.bottom,
                  contentRadius: StyleConstants.overlayFloatingRadius,
                  barrierColor: Colors.transparent,
                  backgroundColor: _pill,
                  contentPadding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 4,
                  ),
                  content: PointerInterceptor(child: moreMenuContent!),
                  child: _pillButton(
                    moreIcon ?? const Icon(Icons.more_horiz, size: 24),
                    null,
                  ),
                )
              else if (onMoreTap != null)
                _pillButton(
                  moreIcon ?? const Icon(Icons.more_horiz, size: 24),
                  onMoreTap,
                )
              else
                const SizedBox(width: _iconButtonSize),
            ],
          ],
        ),
      ),
    );
  }
}

/// 胶囊式标题栏：支持两种用法
/// 1. **普通模式**：作为 [Scaffold.appBar]，在状态栏下方占位显示。
/// 2. **悬浮模式**：使用 [CapsuleStyleOverlayAppBar.connection] 包裹页面内容，标题栏由 [flutter_appbar] 固定在顶部，与内容联动（可固定不随滚动）。
class CapsuleStyleOverlayAppBar extends StatelessWidget
    implements PreferredSizeWidget {
  const CapsuleStyleOverlayAppBar({
    super.key,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreIcon,
    this.backgroundColor,
    this.foregroundColor,
    this.showOnlyBackButton = false,
  });

  final bool showOnlyBackButton;
  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreIcon;
  final Color? backgroundColor;
  final Color? foregroundColor;

  static const double _barContentHeight = 44.0;
  static const double _maxSafeTop = 80.0;

  @override
  Size get preferredSize =>
      const Size.fromHeight(_barContentHeight + _maxSafeTop);

  /// 仅悬浮栏：返回可放入 [Stack] 的透明标题栏（不占位、浮在内容上），用于与 [SlidingUpPanel] 等原有布局搭配。
  /// 若提供 [moreMenuContent]，则「更多」按钮用 [CustomPopup] 包裹，点击后弹出该内容（自动处理位置）。
  static Widget overlayBar({
    Key? key,
    String? title,
    String? subtitle,
    VoidCallback? onBack,
    VoidCallback? onMoreTap,
    Widget? moreMenuContent,
    Widget? moreIcon,
    bool showOnlyBackButton = false,
  }) {
    return _OverlayBarOnly(
      key: key,
      title: title,
      subtitle: subtitle,
      onBack: onBack,
      onMoreTap: onMoreTap,
      moreMenuContent: moreMenuContent,
      moreIcon: moreIcon,
      showOnlyBackButton: showOnlyBackButton,
    );
  }

  /// 悬浮模式：使用 [flutter_appbar] 的 [AppBarConnection]，将标题栏固定在顶部，[child] 在下方。
  /// [child] 会被包在 [CustomScrollView] + [SliverFillRemaining] 中以满足包对可滚动子组件的需求，保证布局正确。
  static Widget connection({
    Key? key,
    required Widget child,
    String? title,
    String? subtitle,
    VoidCallback? onBack,
    VoidCallback? onMoreTap,
    Widget? moreIcon,
    Color? backgroundColor,
    Color? foregroundColor,
    bool showOnlyBackButton = false,
  }) {
    return _CapsuleOverlayConnection(
      key: key,
      child: child,
      title: title,
      subtitle: subtitle,
      onBack: onBack,
      onMoreTap: onMoreTap,
      moreIcon: moreIcon,
      backgroundColor: backgroundColor,
      foregroundColor: foregroundColor,
      showOnlyBackButton: showOnlyBackButton,
    );
  }

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;
    final barColor = backgroundColor ?? const Color(0xFF1C1C1E);
    final isLight = barColor.computeLuminance() > 0.5;
    final pillColor = isLight ? const Color(0xFFE5E5E7) : const Color(0xFF2C2C2E);
    final subtitleFg =
        isLight ? const Color(0xFF636366) : const Color(0xFF8E8E93);

    return Container(
      height: topInset + _barContentHeight,
      decoration: BoxDecoration(
        color: barColor,
        border: Border(
          bottom: BorderSide(
            color: isLight
                ? const Color(0xFFD1D1D6)
                : const Color(0xFF2C2C2E),
            width: 0.5,
          ),
        ),
      ),
      child: Padding(
        padding: EdgeInsets.only(top: topInset),
        child: _CapsuleBarContent(
          showOnlyBackButton: showOnlyBackButton,
          title: title,
          subtitle: subtitle,
          onBack: onBack,
          onMoreTap: onMoreTap,
          moreIcon: moreIcon,
          foregroundColor: foregroundColor ?? const Color(0xFFE5E5E7),
          pillColor: pillColor,
          subtitleFg: subtitleFg,
        ),
      ),
    );
  }
}

class _OverlayBarOnly extends StatelessWidget {
  const _OverlayBarOnly({
    super.key,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreMenuContent,
    this.moreIcon,
    this.showOnlyBackButton = false,
  });

  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreMenuContent;
  final Widget? moreIcon;
  final bool showOnlyBackButton;

  static const double _barContentHeight = 44.0;

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;
    return Positioned(
      top: 0,
      left: 0,
      right: 0,
      child: Container(
        height: topInset + _barContentHeight,
        color: Colors.transparent,
        child: Padding(
          padding: EdgeInsets.only(top: topInset),
          child: _CapsuleBarContent(
            showOnlyBackButton: showOnlyBackButton,
            title: title,
            subtitle: subtitle,
            onBack: onBack,
            onMoreTap: onMoreTap,
            moreMenuContent: moreMenuContent,
            moreIcon: moreIcon,
            foregroundColor: const Color(0xFFE5E5E7),
            pillColor: const Color(0xFF2C2C2E),
            subtitleFg: const Color(0xFF8E8E93),
          ),
        ),
      ),
    );
  }
}

class _CapsuleOverlayConnection extends StatelessWidget {
  const _CapsuleOverlayConnection({
    super.key,
    required this.child,
    this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreIcon,
    this.backgroundColor,
    this.foregroundColor,
    this.showOnlyBackButton = false,
  });

  final Widget child;
  final String? title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreIcon;
  final Color? backgroundColor;
  final Color? foregroundColor;
  final bool showOnlyBackButton;

  static const double _barContentHeight = 44.0;

  @override
  Widget build(BuildContext context) {
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;
    final totalBarHeight = topInset + _barContentHeight;
    // 悬浮模式固定透明底，便于看到下方内容
    const barColor = Colors.transparent;
    const pillColor = Color(0xFF2C2C2E);
    const subtitleFg = Color(0xFF8E8E93);

    final barBody = Container(
      color: barColor,
      child: Padding(
        padding: EdgeInsets.only(top: topInset),
        child: _CapsuleBarContent(
          showOnlyBackButton: showOnlyBackButton,
          title: title,
          subtitle: subtitle,
          onBack: onBack,
          onMoreTap: onMoreTap,
          moreIcon: moreIcon,
          foregroundColor: foregroundColor ?? const Color(0xFFE5E5E7),
          pillColor: pillColor,
          subtitleFg: subtitleFg,
        ),
      ),
    );

    return fappbar.AppBarConnection(
      appBars: [
        fappbar.AppBar(
          behavior: const fappbar.AbsoluteAppBarBehavior(),
          minExtent: totalBarHeight,
          maxExtent: totalBarHeight,
          body: barBody,
        ),
      ],
      child: CustomScrollView(
        slivers: [
          SliverFillRemaining(
            hasScrollBody: false,
            child: child,
          ),
        ],
      ),
    );
  }
}
