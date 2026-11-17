package com.memolanes.oss.dev

import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.plugins.FlutterPlugin

class MainActivity : FlutterActivity() {
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        flutterEngine.plugins.add(DeviceRegionPlugin())
        if (io.flutter.BuildConfig.DEBUG) {
            // The diagnostics implementation is compiled only in the debug source set.
            val plugin = Class.forName("com.memolanes.oss.dev.TraceletMotionDiagnosticsPlugin")
                .getDeclaredConstructor().newInstance() as FlutterPlugin
            flutterEngine.plugins.add(plugin)
        }
    }
}
