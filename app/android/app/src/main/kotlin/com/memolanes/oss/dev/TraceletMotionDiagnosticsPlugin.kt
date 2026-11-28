package com.memolanes.oss.dev

import android.content.Context
import android.util.Log
import com.ikolvi.tracelet.sdk.TraceletSdk
import io.flutter.embedding.engine.plugins.FlutterPlugin
import io.flutter.plugin.common.MethodChannel

class TraceletMotionDiagnosticsPlugin : FlutterPlugin {
    private lateinit var channel: MethodChannel

    override fun onAttachedToEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        val context = binding.applicationContext
        channel = MethodChannel(binding.binaryMessenger, "com.memolanes/tracelet_motion_diagnostics")
        channel.setMethodCallHandler { call, result ->
            if (call.method == "snapshot") {
                try {
                    result.success(snapshot(context, call.argument<Number>("fixAtMs")?.toLong()))
                } catch (error: Exception) {
                    result.error(
                        "tracelet_motion_snapshot_failed",
                        "Reading Tracelet motion diagnostics failed: $error",
                        Log.getStackTraceString(error),
                    )
                }
            } else {
                result.notImplemented()
            }
        }
    }

    override fun onDetachedFromEngine(binding: FlutterPlugin.FlutterPluginBinding) {
        channel.setMethodCallHandler(null)
    }

    private fun snapshot(context: Context, fixAtMs: Long?): Map<String, Any?>? {
        val sdk = TraceletSdk.getInstance(context)
        if (!sdk.isReady) return null
        val coordinator = sdk.smartMotionCoordinator
        val details = mutableMapOf<String, Any?>(
            "accelerometerMoving" to coordinator.isAccelMoving,
            "speedMoving" to coordinator.isSpeedMoving,
            "speedState" to sdk.speedMotionManager.getCurrentState(),
        )
        details.putAll(sdk.locationEngine.getMotionSpeedDiagnostics())
        if (fixAtMs != null) {
            val fix = sdk.locationEngine.getLastLocation()?.takeIf { it.time == fixAtMs }
            details["rawFixMatched"] = fix != null
            details["rawSpeedMps"] = fix
                ?.takeIf { it.hasSpeed() && it.speed.isFinite() && it.speed >= 0 }
                ?.speed?.toDouble()
        }
        return details
    }
}
