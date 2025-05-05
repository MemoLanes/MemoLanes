import 'package:permission_handler/permission_handler.dart';
import 'package:shared_preferences/shared_preferences.dart';

// TODO: move other useage of `SharedPreferences` to this class
class PreferencesManager {
  static const String _notificationKey = 'isCloseNotificationEnabled';

  static Future<bool> getCloseNotificationStatus() async {
    final prefs = await SharedPreferences.getInstance();
    if (!await Permission.notification.isGranted) {
      return false;
    }
    return prefs.getBool(_notificationKey) ?? true;
  }

  static Future<void> setCloseNotificationStatus(bool value) async {
    final prefs = await SharedPreferences.getInstance();
    await prefs.setBool(_notificationKey, value);
  }
}
