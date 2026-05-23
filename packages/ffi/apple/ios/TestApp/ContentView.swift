import SwiftUI

struct ContentView: View {
    @StateObject private var runner = TestRunner()

    var body: some View {
        NavigationView {
            VStack(spacing: 0) {
                summaryHeader
                Divider()
                resultsList
            }
            .navigationTitle("WVB FFI Tests")
            .navigationBarTitleDisplayMode(.large)
            .toolbar {
                ToolbarItem(placement: .navigationBarTrailing) {
                    Button("Run") {
                        Task { await runner.run() }
                    }
                    .disabled(runner.isRunning)
                }
            }
        }
        .navigationViewStyle(.stack)
    }

    @ViewBuilder
    private var summaryHeader: some View {
        HStack(spacing: 6) {
            if runner.isRunning {
                ProgressView()
                Text("Running\u{2026}")
                    .foregroundColor(.secondary)
            } else if !runner.results.isEmpty {
                let passed = runner.results.filter { $0.passed }.count
                let failed = runner.results.filter { !$0.passed }.count
                Text("\(passed) passed")
                    .foregroundColor(.green)
                    .fontWeight(.semibold)
                if failed > 0 {
                    Text("\u{00B7}")
                        .foregroundColor(.secondary)
                    Text("\(failed) failed")
                        .foregroundColor(.red)
                        .fontWeight(.semibold)
                }
            } else {
                Text("Tap Run to start")
                    .foregroundColor(.secondary)
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding()
    }

    private var resultsList: some View {
        List(runner.results) { result in
            HStack(alignment: .top, spacing: 10) {
                Image(systemName: result.passed ? "checkmark.circle.fill" : "xmark.circle.fill")
                    .foregroundColor(result.passed ? .green : .red)
                VStack(alignment: .leading, spacing: 2) {
                    Text(result.name)
                        .font(.system(.body, design: .monospaced))
                    if let error = result.error {
                        Text(error)
                            .font(.caption)
                            .foregroundColor(.red)
                    }
                }
            }
        }
        .listStyle(.plain)
    }
}
