# Performance Characteristics

## Tested Limits

| Component | Scale | Performance | Notes |
|-----------|-------|-------------|-------|
| Layout computation | 100 nodes | < 1ms | Pure Rust, O(V + E) |
| Layout computation | 500 nodes | < 5ms | Barycenter ordering adds ~O(L × N²) |
| Canvas rendering | 100 nodes | ~16ms (60fps) | Canvas2D with Path2D icons |
| Canvas rendering | 500 nodes | ~33ms (30fps) | Consider minimap + viewport culling |
| Queue enqueue | 500 ops | < 1µs/op | In-memory backend |
| Queue claim | 500 ops | < 1µs/op | In-memory backend, linear scan |
| Scheduler cascade | 100-node diamond | < 5s total | Event-driven, includes sleep gaps |

## Recommended Limits

| Scenario | Recommended Max | Reason |
|----------|----------------|--------|
| Interactive editing (60fps) | **100 nodes** | Canvas2D redraws entire scene per frame |
| Static display (no animation) | **500 nodes** | No animation loop overhead |
| With minimap enabled | **200 nodes** | Minimap adds second render pass |
| Concurrent workers | **50 workers** | In-memory queue uses mutex; Postgres/Redis backends scale further |

## Optimization Tips

- **Disable minimap** for graphs under 20 nodes (not needed, saves render time)
- **Use `autoResize: false`** if container size is fixed (avoids ResizeObserver overhead)
- **Batch status updates** — call `updateStatus()` at most once per frame, not per-job
- **Freeze layout** after initial render if users don't need drag — reduces hit-test overhead
- For **500+ nodes**, consider:
  - Viewport culling (only render visible nodes)
  - Reducing `status_icon_radius` to simplify icon rendering
  - Using a WebGL-based renderer instead of Canvas2D

## Running Benchmarks

```bash
# Performance tests (timing output on stderr)
cargo test --test performance -p workflow-graph-queue -- --nocapture

# Integration tests
cargo test --test integration -p workflow-graph-queue
```
