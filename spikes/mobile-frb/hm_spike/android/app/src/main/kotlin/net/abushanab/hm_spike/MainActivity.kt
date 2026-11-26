package net.abushanab.hm_spike

import android.content.Intent
import android.os.SystemClock
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

/** Hands a tapped notification's ask to Dart: at launch (`initial`) or later through `onNewIntent`. */
class MainActivity : FlutterActivity() {
    private var channel: MethodChannel? = null
    private var pending: Map<String, Any>? = null

    private fun askOf(i: Intent?): Map<String, Any>? =
        i?.getStringExtra("ask")?.let { mapOf("ask" to it, "tapped_uptime" to SystemClock.uptimeMillis(), "cold" to (channel == null)) }

    override fun configureFlutterEngine(engine: FlutterEngine) {
        super.configureFlutterEngine(engine)
        pending = askOf(intent)
        channel = MethodChannel(engine.dartExecutor.binaryMessenger, "hm/ask").also { ch ->
            ch.setMethodCallHandler { call, result ->
                when (call.method) {
                    "initial" -> { result.success(pending); pending = null }
                    "uptime" -> result.success(SystemClock.uptimeMillis())
                    "files" -> result.success(filesDir.path)
                    else -> result.notImplemented()
                }
            }
        }
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        askOf(intent)?.let { channel?.invokeMethod("ask", it) }
    }
}
