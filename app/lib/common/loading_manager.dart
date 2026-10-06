import 'dart:async';

import 'package:flutter/material.dart';
import 'package:memolanes/theme/app_colors.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

/// Global loading manager (singleton + reference counting).
class GlobalLoadingManager extends ChangeNotifier {
  GlobalLoadingManager._internal();

  static final GlobalLoadingManager instance = GlobalLoadingManager._internal();

  int _activeWakelockTaskCount = 0;
  int _activeOverlayTaskCount = 0;
  int _activeNavigationBlockCount = 0;
  bool _isLoading = false;
  Timer? _loadingDelayTimer;

  /// Whether the global loading overlay is active.
  bool get isLoading => _isLoading;

  /// Whether navigation must be blocked immediately for an active task.
  bool get isNavigationBlocked => _activeNavigationBlockCount > 0;

  /// Manages the loading lifecycle for async tasks in a unified way.
  ///
  /// - Supports parallel/nested tasks (reference counting).
  /// - Set [blockNavigation] for mutations that must finish before leaving the
  ///   current route. This takes effect immediately, before the delayed loading
  ///   overlay becomes visible.
  Future<T> runWithLoading<T>(
    Future<T> Function() task, {
    bool blockNavigation = false,
  }) async {
    if (blockNavigation) _incrementNavigationBlock();
    try {
      await _incrementWakelock();
      _incrementOverlay();
      try {
        return await task();
      } finally {
        _decrementOverlay();
        await _decrementWakelock();
      }
    } finally {
      if (blockNavigation) _decrementNavigationBlock();
    }
  }

  /// Keeps the device awake for a task without showing the global overlay.
  ///
  /// Use this when the page already provides its own blocking loading UI.
  Future<T> runWithWakelock<T>(Future<T> Function() task) async {
    await _incrementWakelock();
    try {
      return await task();
    } finally {
      await _decrementWakelock();
    }
  }

  Future<void> _incrementWakelock() async {
    final bool wasIdle = _activeWakelockTaskCount == 0;
    _activeWakelockTaskCount += 1;

    if (wasIdle) {
      await WakelockPlus.enable();
    }
  }

  Future<void> _decrementWakelock() async {
    if (_activeWakelockTaskCount > 0) {
      _activeWakelockTaskCount -= 1;
    }
    if (_activeWakelockTaskCount == 0) {
      await WakelockPlus.disable();
    }
  }

  void _incrementOverlay() {
    final bool wasHidden = _activeOverlayTaskCount == 0;
    _activeOverlayTaskCount += 1;

    if (wasHidden) {
      _loadingDelayTimer?.cancel();
      // Delay showing the loading UI a bit to avoid flickering for very fast tasks.
      _loadingDelayTimer = Timer(const Duration(milliseconds: 200), () {
        _isLoading = true;
        notifyListeners();
      });
    }
  }

  void _decrementOverlay() {
    if (_activeOverlayTaskCount > 0) {
      _activeOverlayTaskCount -= 1;
    }
    if (_activeOverlayTaskCount == 0) {
      _loadingDelayTimer?.cancel();
      _loadingDelayTimer = null;
      _isLoading = false;
      notifyListeners();
    }
  }

  void _incrementNavigationBlock() {
    final wasUnblocked = _activeNavigationBlockCount == 0;
    _activeNavigationBlockCount += 1;
    if (wasUnblocked) notifyListeners();
  }

  void _decrementNavigationBlock() {
    if (_activeNavigationBlockCount > 0) {
      _activeNavigationBlockCount -= 1;
    }
    if (_activeNavigationBlockCount == 0) notifyListeners();
  }
}

/// Global loading overlay that wraps the app root.
///
/// Wraps the entire app content with [child] and shows a mask + animation on top
/// when global loading is active.
class GlobalLoadingOverlay extends StatelessWidget {
  final Widget child;

  const GlobalLoadingOverlay({super.key, required this.child});

  @override
  Widget build(BuildContext context) {
    final manager = GlobalLoadingManager.instance;

    return AnimatedBuilder(
      animation: manager,
      builder: (context, _) {
        final isLoading = manager.isLoading;

        return PopScope(
          canPop: !isLoading && !manager.isNavigationBlocked,
          child: Stack(
            alignment: Alignment.topLeft,
            children: [
              child,
              if (isLoading)
                Positioned.fill(
                  child: Stack(
                    alignment: Alignment.center,
                    children: [
                      ModalBarrier(
                        dismissible: false,
                        color: Color.lerp(
                          AppColors.light.shadowColor.withAlpha(0x59),
                          AppColors.dark.shadowColor.withAlpha(0xAD),
                          context.appColors.darkProgress,
                        )!,
                      ),
                      const Center(child: _DefaultLoadingCard()),
                    ],
                  ),
                ),
            ],
          ),
        );
      },
    );
  }
}

/// Blocks route pop (back button / back gesture) while global loading is active.
///
/// Place this widget inside each page route to ensure pop interception works
/// for that route.
class GlobalPopScope extends StatelessWidget {
  final Widget child;

  const GlobalPopScope({super.key, required this.child});

  @override
  Widget build(BuildContext context) {
    final manager = GlobalLoadingManager.instance;
    return AnimatedBuilder(
      animation: manager,
      child: child,
      builder: (context, child) {
        return PopScope(
          canPop: !manager.isLoading && !manager.isNavigationBlocked,
          child: child!,
        );
      },
    );
  }
}

/// Default global loading UI.
class _DefaultLoadingCard extends StatelessWidget {
  const _DefaultLoadingCard();

  @override
  Widget build(BuildContext context) {
    return Container(
      width: 80,
      height: 80,
      decoration: BoxDecoration(
        color: context.appColors.surfaceColor,
        borderRadius: BorderRadius.circular(16),
      ),
      child: const Center(
        child: SizedBox(
          width: 32,
          height: 32,
          child: CircularProgressIndicator(strokeWidth: 3.0),
        ),
      ),
    );
  }
}
