import 'dart:async';

import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

/// Global loading manager (singleton + reference counting).
class GlobalLoadingManager extends ChangeNotifier {
  GlobalLoadingManager._internal();

  static final GlobalLoadingManager instance = GlobalLoadingManager._internal();

  int _activeTaskCount = 0;
  bool _isLoading = false;
  Timer? _loadingDelayTimer;

  /// Whether there is any active loading task.
  bool get isLoading => _isLoading;

  /// Manages the loading lifecycle for async tasks in a unified way.
  ///
  /// - Supports parallel/nested tasks (reference counting).
  Future<T> runWithLoading<T>(Future<T> Function() task) async {
    _increment();
    try {
      return await task();
    } finally {
      _decrement();
    }
  }

  void _increment() {
    if (_activeTaskCount == 0) {
      unawaited(WakelockPlus.enable());
      _loadingDelayTimer?.cancel();
      // Delay showing the loading UI a bit to avoid flickering for very fast tasks.
      _loadingDelayTimer = Timer(const Duration(milliseconds: 200), () {
        _isLoading = true;
        notifyListeners();
      });
    }
    _activeTaskCount += 1;
  }

  void _decrement() {
    if (_activeTaskCount > 0) {
      _activeTaskCount -= 1;
    }
    if (_activeTaskCount == 0) {
      unawaited(WakelockPlus.disable());
      _loadingDelayTimer?.cancel();
      _loadingDelayTimer = null;
      _isLoading = false;
      notifyListeners();
    }
  }
}

/// Global loading overlay that wraps the app root.
///
/// Wraps the entire app content with [child] and shows a mask + animation on top
/// when global loading is active.
class GlobalLoadingOverlay extends StatelessWidget {
  final Widget child;

  const GlobalLoadingOverlay({
    super.key,
    required this.child,
  });

  @override
  Widget build(BuildContext context) {
    final manager = GlobalLoadingManager.instance;

    return AnimatedBuilder(
      animation: manager,
      builder: (context, _) {
        final isLoading = manager.isLoading;

        return PopScope(
          canPop: !isLoading,
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
                        color: StyleConstants.loadingMaskColor,
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
          canPop: !manager.isLoading,
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
        color: Colors.white,
        borderRadius: BorderRadius.circular(16),
      ),
      child: const Center(
        child: SizedBox(
          width: 32,
          height: 32,
          child: CircularProgressIndicator(
            strokeWidth: 3.0,
          ),
        ),
      ),
    );
  }
}
