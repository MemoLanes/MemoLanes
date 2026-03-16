import 'dart:async';

import 'package:flutter/material.dart';

/// 全局 Loading 管理器（单例 + 引用计数）
class GlobalLoadingManager extends ChangeNotifier {
  GlobalLoadingManager._internal();

  static final GlobalLoadingManager instance = GlobalLoadingManager._internal();

  int _activeTaskCount = 0;
  int _currentMaxPriority = 0;

  /// 当前是否有正在进行的 loading 任务
  bool get isLoading => _activeTaskCount > 0;

  /// 当前最高优先级（暂未用于切换样式，但为未来扩展预留）
  int get currentPriority => _currentMaxPriority;

  /// 统一管理异步任务的 loading 生命周期
  ///
  /// - 支持并行 / 嵌套任务（引用计数）
  /// - 支持超时（超时只影响结果，不影响计数的正确回收）
  Future<T> runWithLoading<T>(
    Future<T> Function() task, {
    int priority = 0,
    Duration? timeout,
    // 任务开始后多久才考虑显示 Loading（防止闪烁）
    Duration minDelayBeforeShow = const Duration(milliseconds: 300),
  }) async {
    var taskCompletedEarly = false;

    // 先启动任务
    final future = task();
    future.whenComplete(() {
      taskCompletedEarly = true;
    });

    // 等待一小段时间，避免闪烁
    if (minDelayBeforeShow > Duration.zero) {
      await Future.delayed(minDelayBeforeShow);
    }

    // 任务已经完成，就不展示 loading，直接返回结果
    if (taskCompletedEarly) {
      return timeout == null ? await future : await future.timeout(timeout);
    }

    // 任务仍在进行，增加引用计数并展示全局 loading
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

/// 根部包裹用的全局 Loading Overlay
///
/// 将整个应用内容包在 [child] 之上，当有全局 loading 时在最上层展示遮罩和动画。
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

/// 默认的全局 loading 样式
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

