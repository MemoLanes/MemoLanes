import 'dart:async';

/// Gives queued GPS fixes time to arrive after a resume or a delayed tick.
class JourneyIdleTimer {
  static const _interval = Duration(minutes: 30);
  static const _resumeGrace = Duration(minutes: 1);
  static const _lateTolerance = Duration(minutes: 1);

  final Future<void> Function(bool Function() isCurrent) _check;
  final DateTime Function() _now;
  Timer? _timer;
  late DateTime _nextCheckAt;

  JourneyIdleTimer(this._check, {DateTime Function()? now})
    : _now = now ?? DateTime.now;

  void restart() => _schedule(_interval);

  void onResume() {
    if (_timer != null && _nextCheckAt.difference(_now()) < _resumeGrace) {
      _schedule(_resumeGrace);
    }
  }

  void _schedule(Duration delay) {
    cancel();
    _nextCheckAt = _now().add(delay);
    final expiresAt = _nextCheckAt.add(_lateTolerance);
    _timer = Timer(delay, () async {
      final current = _timer;
      // Recheck after waiting for the recording queue, which can itself suspend.
      bool isCurrent() => _timer == current && !_now().isAfter(expiresAt);
      try {
        if (isCurrent()) await _check(isCurrent);
      } finally {
        if (_timer == current) {
          _schedule(_now().isAfter(expiresAt) ? _resumeGrace : _interval);
        }
      }
    });
  }

  void cancel() {
    _timer?.cancel();
    _timer = null;
  }
}
