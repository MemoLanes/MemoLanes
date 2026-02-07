import 'package:flutter/material.dart';
import 'package:memolanes/common/component/custom_popup.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:pointer_interceptor/pointer_interceptor.dart';

class CapsuleBarConstants {
  CapsuleBarConstants._();

  static const double barContentHeight = 44.0;
  static const double barBottomInset = 4.0;
  static const double maxSafeTop = 80.0;
  static const double pillRadius = 18.0;
  static const double iconButtonSize = 36.0;

  static const Color defaultForeground = Color(0xFFE5E5E7);
  static const Color defaultPill = Color(0xFF2C2C2E);
  static const Color defaultSubtitleFg = Color(0xFF8E8E93);
  static const Color defaultBackground = Color(0xFF1C1C1E);
  static const Color barBorderColor = Color(0xFF2C2C2E);
  static const Color barBorderColorLight = Color(0xFFD1D1D6);
  static const Color lightPillBackground = Color(0xFFE5E5E7);
  static const Color subtitleColorLight = Color(0xFF636366);
}

/// Capsule-style bar content: back button, optional title pill, optional more button.
/// Does not include safe area or outer container; used inside [CapsuleStyleAppBar] or [CapsuleStyleOverlayAppBar].
class CapsuleBarContent extends StatelessWidget {
  const CapsuleBarContent({
    super.key,
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

  /// If set, the more button is wrapped with [CustomPopup] and shows this content on tap.
  final Widget? moreMenuContent;
  final Widget? moreIcon;
  final Color? foregroundColor;
  final Color? pillColor;
  final Color? subtitleFg;

  Color get _fg => foregroundColor ?? CapsuleBarConstants.defaultForeground;
  Color get _pill => pillColor ?? CapsuleBarConstants.defaultPill;
  Color get _subFg => subtitleFg ?? CapsuleBarConstants.defaultSubtitleFg;

  Widget _pillButton(Widget icon, VoidCallback? onPressed) {
    return Material(
      color: _pill,
      shape: const CircleBorder(),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onPressed,
        customBorder: const CircleBorder(),
        child: SizedBox(
          width: CapsuleBarConstants.iconButtonSize,
          height: CapsuleBarConstants.iconButtonSize,
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
      height: CapsuleBarConstants.barContentHeight,
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
                      borderRadius:
                          BorderRadius.circular(CapsuleBarConstants.pillRadius),
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
                const SizedBox(width: CapsuleBarConstants.iconButtonSize),
            ],
          ],
        ),
      ),
    );
  }
}
