import 'dart:async';

import 'package:flutter_fgbg/flutter_fgbg.dart';
import 'package:memolanes/common/log.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

class AppLifecycleService {
  AppLifecycleService({
    Stream<FGBGType>? events,
    Future<void> Function()? reloadResource,
    void Function()? freeResource,
  }) : _events = events ?? FGBGEvents.instance.stream,
       _reloadResourceOverride = reloadResource,
       _freeResourceOverride = freeResource;

  static final AppLifecycleService instance = AppLifecycleService();

  final Stream<FGBGType> _events;
  final Future<void> Function()? _reloadResourceOverride;
  final void Function()? _freeResourceOverride;

  StreamSubscription? _sub;
  Timer? _freeResourceCountdown;
  bool _countdownCanceled = false;
  Future<void> _serializedWork = Future<void>.value();

  bool get isRunning => _sub != null;

  void start({
    Future<void> Function()? onForeground,
    Future<void> Function()? onBackground,
  }) {
    if (_sub != null) return;

    _sub = _events.listen((event) {
      final triggerTime = DateTime.now();

      if (event == FGBGType.background) {
        log.info(
          '[AppLifecycleService][$triggerTime] Background event received.',
        );
        _countdownCanceled = false;
        _startFreeResourceCountdown();
        if (onBackground != null) {
          _enqueueSerializedWork(
            () => _runHook('Background handling', onBackground),
          );
        }
      } else if (event == FGBGType.foreground) {
        log.info(
          '[AppLifecycleService][$triggerTime] Foreground event received.',
        );
        _reset();
        _enqueueSerializedWork(() async {
          await _runHook('Foreground resource reload', _reloadResource);
          if (onForeground != null) {
            await _runHook('Foreground recovery', onForeground);
          }
        });
      }
    });
  }

  void _enqueueSerializedWork(Future<void> Function() work) {
    _serializedWork = _serializedWork.then((_) => work());
    unawaited(_serializedWork);
  }

  Future<void> _runHook(
    String description,
    Future<void> Function() hook,
  ) async {
    try {
      await hook();
    } catch (error, stackTrace) {
      log.error(
        '[AppLifecycleService] $description failed: $error',
        stackTrace,
      );
    }
  }

  void _reset() {
    _freeResourceCountdown?.cancel();
    _freeResourceCountdown = null;
    _countdownCanceled = true;
  }

  void _startFreeResourceCountdown() {
    if (_freeResourceCountdown != null) {
      _freeResourceCountdown!.cancel();
      _freeResourceCountdown = null;
    }

    // try to free resource after 2 minutes in background
    _freeResourceCountdown = Timer(const Duration(seconds: 2 * 60), () {
      // delay a bit to avoid flicker with foreground event
      Future.delayed(const Duration(milliseconds: 500), () {
        if (!_countdownCanceled) {
          _freeResource();
        } else {
          final triggerTime = DateTime.now();
          log.info(
            '[AppLifecycleService][$triggerTime] free resource task skipped.',
          );
        }
      });
    });
  }

  void _freeResource() {
    final triggerTime = DateTime.now();
    log.info('[AppLifecycleService][$triggerTime] Try to free resource.');
    final freeResource = _freeResourceOverride;
    if (freeResource == null) {
      api.freeResourceForLongTimeBackground();
    } else {
      freeResource();
    }
  }

  Future<void> _reloadResource() {
    // TODO: would be nice if we can display a loading indicator if this is taking very long
    return _reloadResourceOverride?.call() ?? api.reloadResourceForForeground();
  }

  Future<void> stop() async {
    await _sub?.cancel();
    _sub = null;
    _reset();
    await _serializedWork;
  }
}
