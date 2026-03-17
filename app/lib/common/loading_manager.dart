import 'dart:async';

import 'package:flutter/material.dart';

/// Global loading manager (singleton + reference counting).
class GlobalLoadingManager extends ChangeNotifier {
  GlobalLoadingManager._internal();

  static final GlobalLoadingManager instance = GlobalLoadingManager._internal();

  int _activeTaskCount = 0;
  int _currentMaxPriority = 0;

  /// Whether there is any active loading task.
  bool get isLoading => _activeTaskCount > 0;

  /// Current max priority (reserved for future style switching/extension).
  int get currentPriority => _currentMaxPriority;

  /// Manages the loading lifecycle for async tasks in a unified way.
  ///
  /// - Supports parallel/nested tasks (reference counting).
  /// - Supports timeout (timeout only affects the result, not counter cleanup).
  Future<T> runWithLoading<T>(
    Future<T> Function() task, {
    int priority = 0,
    Duration? timeout,
    // How long to wait before showing loading (prevents flicker).
    Duration minDelayBeforeShow = const Duration(milliseconds: 300),
  }) async {
    var taskCompletedEarly = false;

    final future = task();
    future.whenComplete(() {
      taskCompletedEarly = true;
    });

    if (minDelayBeforeShow > Duration.zero) {
      await Future.delayed(minDelayBeforeShow);
    }

    if (taskCompletedEarly) {
      return timeout == null ? await future : await future.timeout(timeout);
    }

    _increment(priority: priority);
    try {
      if (timeout != null) {
        return await future.timeout(timeout);
      }
      return await future;
    } finally {
      _decrement();
    }
  }

  void _increment({required int priority}) {
    _activeTaskCount += 1;
    if (priority > _currentMaxPriority) {
      _currentMaxPriority = priority;
    }
    notifyListeners();
  }

  void _decrement() {
    if (_activeTaskCount > 0) {
      _activeTaskCount -= 1;
    }
    if (_activeTaskCount == 0) {
      _currentMaxPriority = 0;
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

        return Stack(
          alignment: Alignment.topLeft,
          children: [
            child,
            if (isLoading)
              Positioned.fill(
                child: IgnorePointer(
                  ignoring: false,
                  child: Container(
                    color: Colors.black.withOpacity(0.35),
                    child: const Center(
                      child: _DefaultLoadingCard(),
                    ),
                  ),
                ),
              ),
          ],
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
