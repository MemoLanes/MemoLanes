import 'package:flutter/foundation.dart';
import 'package:logging/logging.dart';
import 'package:memolanes/src/rust/api/api.dart' as api;

void initLog() {
  if (kDebugMode) {
    api.subscribeToLogStream().listen((log) {
      if (kDebugMode) {
        print(log);
      }
    });
  }
  Logger.root.level = Level.INFO;
  Logger.root.onRecord.listen((rec) {
    api.LogLevel logLevel = switch (rec.level) {
      Level.INFO => api.LogLevel.info,
      Level.WARNING => api.LogLevel.warn,
      Level.SEVERE => api.LogLevel.error,
      _ => api.LogLevel.info,
    };
    api.writeLog(message: rec.message, level: logLevel);
    if (kDebugMode && rec.level == Level.SEVERE && rec.stackTrace != null) {
      if (kDebugMode) {
        print(rec.stackTrace);
      }
    }
  });
}

class _Log {
  final Logger _logger = Logger('App');

  void info(String message) {
    _logger.info(message);
  }

  void warning(String message) {
    _logger.warning(message);
  }

  void error(String message, [StackTrace? stackTrace]) {
    _logger.severe(message, null, stackTrace);
  }
}

final log = _Log();
