package dev.wvb.testapp

import android.graphics.Color
import android.graphics.Rect
import android.os.Bundle
import android.text.SpannableStringBuilder
import android.text.style.ForegroundColorSpan
import androidx.appcompat.app.AppCompatActivity
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.lifecycleScope
import dev.wvb.testapp.databinding.ActivityMainBinding
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class MainActivity : AppCompatActivity() {
    private lateinit var binding: ActivityMainBinding

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        binding = ActivityMainBinding.inflate(layoutInflater)
        setContentView(binding.root)

        applyWindowInsets()
        binding.btnRun.setOnClickListener { runTests() }
    }

    /**
     * targetSdk 35 makes Android 15+ draw the window edge-to-edge, so without this the button sits
     * under the status bar / camera cutout and taps on its centre never reach it.
     */
    private fun applyWindowInsets() {
        val root = binding.root
        val base = Rect(root.paddingLeft, root.paddingTop, root.paddingRight, root.paddingBottom)
        ViewCompat.setOnApplyWindowInsetsListener(root) { view, windowInsets ->
            val insets = windowInsets.getInsets(
                WindowInsetsCompat.Type.systemBars() or WindowInsetsCompat.Type.displayCutout()
            )
            view.setPadding(
                base.left + insets.left,
                base.top + insets.top,
                base.right + insets.right,
                base.bottom + insets.bottom
            )
            windowInsets
        }
    }

    private fun runTests() {
        binding.btnRun.isEnabled = false
        binding.tvSummary.text = "Running…"
        binding.tvOutput.text = ""

        lifecycleScope.launch {
            try {
                val results = TestRunner(applicationContext).run()
                displayResults(results)
            } catch (e: Throwable) {
                withContext(Dispatchers.Main) {
                    binding.tvSummary.text = "Fatal error"
                    binding.tvSummary.setTextColor(Color.parseColor("#C62828"))
                    binding.tvOutput.text = "${e.javaClass.name}: ${e.message}\n\n${e.stackTraceToString()}"
                }
            } finally {
                withContext(Dispatchers.Main) {
                    binding.btnRun.isEnabled = true
                }
            }
        }
    }

    private suspend fun displayResults(results: List<TestResult>) = withContext(Dispatchers.Main) {
        val passed = results.count { it.passed }
        val failed = results.count { !it.passed }

        binding.tvSummary.text = "$passed passed, $failed failed"
        binding.tvSummary.setTextColor(if (failed == 0) Color.parseColor("#2E7D32") else Color.parseColor("#C62828"))

        val sb = SpannableStringBuilder()
        for (result in results) {
            val lineStart = sb.length
            if (result.passed) {
                sb.append("✓ ${result.name}\n")
                sb.setSpan(ForegroundColorSpan(Color.parseColor("#2E7D32")), lineStart, sb.length - 1, 0)
            } else {
                sb.append("✗ ${result.name}\n")
                sb.setSpan(ForegroundColorSpan(Color.parseColor("#C62828")), lineStart, sb.length - 1, 0)
                result.error?.let { err ->
                    val errStart = sb.length
                    sb.append("  $err\n")
                    sb.setSpan(ForegroundColorSpan(Color.parseColor("#B71C1C")), errStart, sb.length - 1, 0)
                }
            }
        }
        binding.tvOutput.text = sb
    }
}
