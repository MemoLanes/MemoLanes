import 'package:flutter/material.dart';

/// 胶囊式标题栏：左侧圆形返回按钮、中间胶囊标题区（主标题 + 可选副标题）、右侧圆形更多按钮。
/// 适配深色背景（如 scaffoldBackgroundColor 0xFF141414）。
class CapsuleStyleAppBar extends StatelessWidget implements PreferredSizeWidget {
  const CapsuleStyleAppBar({
    super.key,
    required this.title,
    this.subtitle,
    this.onBack,
    this.onMoreTap,
    this.moreIcon,
    this.backgroundColor,
    this.foregroundColor,
  });

  final String title;
  final String? subtitle;
  final VoidCallback? onBack;
  final VoidCallback? onMoreTap;
  final Widget? moreIcon;
  final Color? backgroundColor;
  final Color? foregroundColor;

  /// 标题栏底色，略浅于页面背景 0xFF141414，使标题栏单独呈现
  static const Color _defaultBackground = Color(0xFF1C1C1E);
  static const Color _pillBackground = Color(0xFF2C2C2E);
  /// 标题栏底部分隔线色（深色栏）
  static const Color _barBorderColor = Color(0xFF2C2C2E);
  /// 标题栏底部分隔线色（浅色栏）
  static const Color _barBorderColorLight = Color(0xFFD1D1D6);
  static const Color _defaultForeground = Color(0xFFE5E5E7);
  static const Color _subtitleColor = Color(0xFF8E8E93);

  /// 标题栏内容区域高度，不包含安全区
  static const double _barContentHeight = 44.0;
  /// 内容区底部边距，与顶部形成上下统一的小留白
  static const double _barBottomInset = 4.0;
  /// 预留给安全区的最大高度（挖孔/刘海等），总高度 = _barContentHeight + _barBottomInset + _maxSafeTop
  static const double _maxSafeTop = 80.0;
  static const double _pillRadius = 18.0;
  static const double _iconButtonSize = 36.0;

  /// 浅色标题栏下的胶囊/按钮背景（与 inversePrimary 等搭配）
  static const Color _lightPillBackground = Color(0xFFE5E5E7);

  @override
  Size get preferredSize =>
      const Size.fromHeight(_barContentHeight + _barBottomInset + _maxSafeTop);

  Color get _bg => backgroundColor ?? _defaultBackground;
  Color get _fg => foregroundColor ?? _defaultForeground;
  bool get _isLightBar => _bg.computeLuminance() > 0.5;
  Color get _pillColor =>
      _isLightBar ? _lightPillBackground : _pillBackground;
  Color get _subtitleFg =>
      _isLightBar ? const Color(0xFF636366) : _subtitleColor;

  Widget _buildPillButton({
    required Widget icon,
    required VoidCallback? onPressed,
  }) {
    return Material(
      color: _pillColor,
      shape: const CircleBorder(),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onPressed,
        customBorder: const CircleBorder(),
        child: SizedBox(
          width: _iconButtonSize,
          height: _iconButtonSize,
          child: Center(child: IconTheme.merge(
            data: IconThemeData(color: _fg, size: 20),
            child: icon,
          )),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final effectiveOnBack = onBack ?? () => Navigator.maybePop(context);
    final padding = MediaQuery.paddingOf(context);
    final topInset = padding.top * 0.8;

    // 总高度 = 顶部留白 + 内容高度 + 底部边距，上下留白统一且紧凑
    return Container(
      height: topInset + _barContentHeight + _barBottomInset,
      decoration: BoxDecoration(
        color: _bg,
        border: Border(
          bottom: BorderSide(
            color: _isLightBar ? _barBorderColorLight : _barBorderColor,
            width: 0.5,
          ),
        ),
      ),
      child: Padding(
        padding: EdgeInsets.only(top: topInset, bottom: _barBottomInset),
        child: SizedBox(
          height: _barContentHeight,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 8.0),
            child: Row(
              children: [
                _buildPillButton(
                  icon: const Icon(Icons.arrow_back_ios_new, size: 20),
                  onPressed: effectiveOnBack,
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: Center(
                    child: Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: 14,
                        vertical: 5,
                      ),
                      decoration: BoxDecoration(
                        color: _pillColor,
                        borderRadius:
                            BorderRadius.circular(_pillRadius),
                      ),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        mainAxisAlignment: MainAxisAlignment.center,
                        children: [
                          Text(
                            title,
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
                                color: _subtitleFg,
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
                if (onMoreTap != null)
                  _buildPillButton(
                    icon: moreIcon ??
                        const Icon(Icons.more_horiz, size: 24),
                    onPressed: onMoreTap,
                  )
                else
                  const SizedBox(width: _iconButtonSize),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
