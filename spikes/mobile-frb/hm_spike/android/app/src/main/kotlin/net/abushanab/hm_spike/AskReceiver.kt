package net.abushanab.hm_spike

import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.app.Notification

/** Spike stand-in for a push: `am broadcast -n net.abushanab.hm_spike/.AskReceiver --es ask <id>` posts the
 *  content-free alert the relay would send ("A session needs you"); tapping it opens the app on the request. */
class AskReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val ask = intent.getStringExtra("ask") ?: return
        val nm = context.getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel("needs", "Needs you", NotificationManager.IMPORTANCE_HIGH))
        val open = Intent(context, MainActivity::class.java).putExtra("ask", ask)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_SINGLE_TOP)
        val pi = PendingIntent.getActivity(context, 1, open, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        val n = Notification.Builder(context, "needs").setSmallIcon(android.R.drawable.ic_dialog_alert)
            .setContentTitle("A session needs you").setContentText("Spike host").setContentIntent(pi).setAutoCancel(true)
            .setCategory(Notification.CATEGORY_CALL).build()
        nm.notify(1, n)
    }
}
