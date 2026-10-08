import 'package:flutter/foundation.dart';

/// Preserves map input order while the native editor processes each mutation.
class JourneyEditorOperationQueue extends ChangeNotifier {
  Future<void> _tail = Future<void>.value();
  int _pending = 0;
  bool _disposed = false;

  bool get isBusy => _pending > 0;

  Future<void> run(Future<void> Function() operation) {
    if (_disposed) return Future<void>.value();
    _pending += 1;
    notifyListeners();
    final result = _tail
        .then((_) async {
          if (!_disposed) await operation();
        })
        .whenComplete(() {
          _pending -= 1;
          if (!_disposed) notifyListeners();
        });
    // A failed mutation must not prevent later input from being processed.
    _tail = result.then<void>((_) {}, onError: (Object _, StackTrace _) {});
    return result;
  }

  @override
  void dispose() {
    _disposed = true;
    super.dispose();
  }
}
