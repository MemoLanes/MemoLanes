import 'dart:async';

import 'package:flutter/material.dart';
import 'package:memolanes/constants/style_constants.dart';
import 'package:wakelock_plus/wakelock_plus.dart';

/// Global loading manager (singleton + reference counting).
class GlobalLoadingManager extends ChangeNotifier {
  GlobalLoadingManager._internal();

  static final GlobalLoadingManager instance = GlobalLoadingManager._internal();

  int _activeTaskCount = 0;

  /// Whether there is any active loading task.
  bool get isLoading => _activeTaskCount > 0;

  /// Manages the loading lifecycle for async tasks in a unified way.
  ///
  /// - Supports parallel/nested tasks (reference counting).
  /// - Supports timeout (timeout only affects the result, not counter cleanup).
  Future<T> runWithLoading<T>(
    Future<T> Function() task, {
    Duration? timeout,
    // How long to wait before showing loading (prevents flicker).
    Duration minDelayBeforeShow = const Duration(milliseconds: 200),
  }) async {
    final Future<T> future = Future<T>.sync(task);

    if (minDelayBeforeShow <= Duration.zero) {
      _increment();
      try {
        return timeout == null ? await future : await future.timeout(timeout);
      } finally {
        _decrement();
      }
    }

    final Object delayToken = Object();
    final delay = Future<void>.delayed(minDelayBeforeShow);
    final first = await Future.any<Object?>(<Future<Object?>>[
      future.then<Object?>((v) => v),
      delay.then<Object?>((_) => delayToken),
    ]);

    // If task finished before the delay, return immediately without showing loading.
    if (!identical(first, delayToken)) {
      // Still apply timeout here for compatibility with the old behavior.
      return timeout == null ? (first as T) : await future.timeout(timeout);
    }

    _increment();
    try {
      return timeout == null ? await future : await future.timeout(timeout);
    } finally {
      _decrement();
    }
  }

  void _increment() {
    if (_activeTaskCount == 0) {
      unawaited(WakelockPlus.enable());
    }
    _activeTaskCount += 1;
    notifyListeners();
  }

  void _decrement() {
    if (_activeTaskCount > 0) {
      _activeTaskCount -= 1;
    }
    if (_activeTaskCount == 0) {
      unawaited(WakelockPlus.disable());
    }
    notifyListeners();
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

/// A [MaterialPageRoute] that blocks route pop while global loading is active.
///
/// This is a low-intrusion way to apply "loading blocks back gesture/button"
/// to all routes created via [MaterialPageRoute] in the app.
class GlobalLoadingMaterialPageRoute<T> extends MaterialPageRoute<T> {
  GlobalLoadingMaterialPageRoute({
    required WidgetBuilder builder,
    super.settings,
    super.maintainState,
    super.fullscreenDialog,
    super.allowSnapshotting,
  }) : super(
          builder: (context) {
            final manager = GlobalLoadingManager.instance;
            return AnimatedBuilder(
              animation: manager,
              child: Builder(builder: builder),
              builder: (context, child) {
                return PopScope(
                  canPop: !manager.isLoading,
                  child: child!,
                );
              },
            );
          },
        );
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
