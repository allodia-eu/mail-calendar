// The Android MX resolver behind the shared detection engine's DNS port. It uses
// android.net.DnsResolver.rawQuery on the device's active network, so the user's real DNS
// configuration (private DNS, a VPN) is honoured, the Rust core ships no resolver on
// purpose. The answer's AD bit is surfaced as authenticData, but is not used in any trust
// decision today (DNS-derived configs are trusted on CA-validated TLS), it is reserved for
// a future opt-in "require DNSSEC" setting.
//
// resolveMx is called by the core on a blocking worker thread (it wraps the call in a task
// with its own timeout); the local latch just bounds a hung platform lookup. A WebView-style
// renderer isn't involved, but like it this needs a real device, the JVM suite covers the
// wire codec (DnsMessage) instead.
package eu.allodia.mailcal

import android.content.Context
import android.net.ConnectivityManager
import android.net.DnsResolver
import android.os.CancellationSignal
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executor
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
import uniffi.mailcal_bindings.DnsException
import uniffi.mailcal_bindings.MxRecord
import uniffi.mailcal_bindings.MxResolution
import uniffi.mailcal_bindings.MxResolver
import uniffi.mailcal_bindings.SrvRecord
import uniffi.mailcal_bindings.SrvResolution

internal class AndroidMxResolver(context: Context) : MxResolver {
    private val appContext = context.applicationContext

    override fun resolveMx(domain: String): MxResolution {
        val query = try {
            DnsMessage.buildMxQuery(domain)
        } catch (e: IllegalArgumentException) {
            throw DnsException.Lookup(e.message ?: "invalid domain")
        }
        val answer = try {
            DnsMessage.parseMxResponse(rawQuery(query))
        } catch (e: DnsException) {
            throw e
        } catch (e: Exception) {
            throw DnsException.Lookup("malformed DNS answer: ${e.message}")
        }
        return MxResolution(
            records = answer.records.map { MxRecord(it.preference.toUShort(), it.exchange) },
            authenticData = answer.authenticData,
        )
    }

    override fun resolveSrv(name: String): SrvResolution {
        val query = try {
            DnsMessage.buildSrvQuery(name)
        } catch (e: IllegalArgumentException) {
            throw DnsException.Lookup(e.message ?: "invalid name")
        }
        val answer = try {
            DnsMessage.parseSrvResponse(rawQuery(query))
        } catch (e: DnsException) {
            throw e
        } catch (e: Exception) {
            throw DnsException.Lookup("malformed DNS answer: ${e.message}")
        }
        return SrvResolution(
            records = answer.records.map {
                SrvRecord(it.priority.toUShort(), it.weight.toUShort(), it.port.toUShort(), it.target)
            },
            authenticData = answer.authenticData,
        )
    }

    private fun rawQuery(query: ByteArray): ByteArray {
        val connectivity = appContext.getSystemService(ConnectivityManager::class.java)
        val network = connectivity?.activeNetwork ?: throw DnsException.Lookup("no active network")

        val latch = CountDownLatch(1)
        val answer = AtomicReference<ByteArray>()
        val failure = AtomicReference<String>()
        val signal = CancellationSignal()
        val direct = Executor { it.run() }

        DnsResolver.getInstance().rawQuery(
            network,
            query,
            DnsResolver.FLAG_EMPTY,
            direct,
            signal,
            object : DnsResolver.Callback<ByteArray> {
                override fun onAnswer(response: ByteArray, rcode: Int) {
                    if (rcode == 0) answer.set(response) else failure.set("dns rcode $rcode")
                    latch.countDown()
                }

                override fun onError(error: DnsResolver.DnsException) {
                    failure.set(error.message ?: "dns error")
                    latch.countDown()
                }
            },
        )

        if (!latch.await(LOOKUP_TIMEOUT_SECONDS, TimeUnit.SECONDS)) {
            signal.cancel()
            throw DnsException.Lookup("dns lookup timed out")
        }
        failure.get()?.let { throw DnsException.Lookup(it) }
        return answer.get() ?: throw DnsException.Lookup("empty dns answer")
    }

    private companion object {
        const val LOOKUP_TIMEOUT_SECONDS = 5L
    }
}
