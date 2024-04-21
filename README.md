# tiny_wgpu

Helper library to reduce the amount of boilerplate code when using `wgpu`.

## Implementation notes

Buffers, textures, pipelines, etc. are stored in `HashMap<&str, T>` objects, so each buffer/pipeline is associated with a string slice label.

## Example usage:
```rs

let compute: Arc<Compute> = Compute::init().await;

...

let program = ComputeProgram::new(compute.clone());

program.add_module("my_shader", wgpu::include_wgsl!("path/to/shader.wgsl"));

program.add_texture(
    "my_texture", 
    TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC, 
    wgpu::TextureFormat::Rgba8Unorm, 
    wgpu::Extent3d { ... }
);

program.add_bind_group("bind_group_name", &[
    BindGroupItem::Texture { label: "my_texture" }
]);

program.add_render_pipelines(
    "my_shader", 
    &["bind_group_name"], 
    &[("my_pipeline_name", ("vertex_shader_name", "fragment_shader_name"))],
    &[], // Any push constants
    &[Some(wgpu::TextureFormat::Rgba8Unorm.into())], // Target formats
    &[], // Any vertex buffer layouts
);

...

let mut encoder = program.compute.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
    label: None
});

{
    let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: None,
        color_attachments: &[
            Some(wgpu::RenderPassColorAttachment { 
                view: &self.program.texture_views["my_texture"], 
                resolve_target: None, 
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store
                },
            })
        ],
        ..Default::default()
    });

    rpass.set_pipeline(&self.program.render_pipelines["my_pipeline_name"]);
    rpass.set_bind_group(0, &self.program.bind_groups["bind_group_name"], &[]);
    rpass.draw(0..3, 0..1);
}
```