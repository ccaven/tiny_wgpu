# tiny_wgpu

Helper library to reduce the amount of boilerplate code when using `wgpu`.

## Usage

Add these lines to your `Cargo.toml`:
```toml
tiny_wgpu = "0.1.0"
```

## Implementation notes

Buffers, textures, pipelines, etc. are stored in `HashMap<&str, T>` objects, so each buffer/pipeline is associated with a string slice label.

See `examples/compute.rs` for a simple compute shader example and `examples/window.rs` for a vertex/fragment shader example using `winit`.