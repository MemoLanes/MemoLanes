package com.memolanes.oss.dev

import android.app.LocaleManager
import android.content.res.Resources
import android.os.Build
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "com.memolanes/device_region")
            .setMethodCallHandler { call, result ->
                if (call.method != "getRegion") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val region = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                    val locales = getSystemService(LocaleManager::class.java)?.systemLocales
                    if (locales == null || locales.isEmpty) null else locales.get(0).country
                } else {
                    val configuration = Resources.getSystem().configuration
                    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.N) {
                        val locales = configuration.locales
                        if (locales.isEmpty) null else locales.get(0).country
                    } else {
                        @Suppress("DEPRECATION")
                        configuration.locale.country
                    }
                }
                result.success(region)
            }
    }
}
