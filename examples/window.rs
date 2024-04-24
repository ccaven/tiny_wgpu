use pollster::FutureExt;
use winit::{event::{Event, WindowEvent}, event_loop::EventLoop, window::Window};
fn main() {

    let event_loop = EventLoop::new().unwrap();
    let window = Window::new(&event_loop).unwrap();

    let compute = tiny_wgpu::Compute::init().block_on();

    let surface = compute.instance.create_surface(&window).unwrap();

    let mut program = tiny_wgpu::ComputeProgram::new(compute.clone());

    program.add_module("window", wgpu::include_wgsl!("window.wgsl"));

    let swapchain_capabilities = surface.get_capabilities(&compute.adapter);
    let swapchain_format = swapchain_capabilities.formats[0];
    program.add_render_pipelines("window", &[], &[("window", ("vs_main", "fs_main"))], &[], &[Some(swapchain_format.into())], &[]);

    let mut config = surface
        .get_default_config(&compute.adapter, 400, 400)
        .unwrap();
    
    surface.configure(&compute.device, &config);

    let window = &window;

    event_loop.run(move |event, target| {

        if let Event::WindowEvent { window_id: _window_id, event } = event {
            match event {
                WindowEvent::Resized(new_size) => {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    surface.configure(&compute.device, &config);
                    window.request_redraw();
                },
                WindowEvent::RedrawRequested => {
                    let frame = surface.get_current_texture().unwrap();
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = compute.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: None
                    });

                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: None,
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color::GREEN),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                        rpass.set_pipeline(&program.render_pipelines["window"]);
                        rpass.draw(0..3, 0..1);
                    }

                    compute.queue.submit(Some(encoder.finish()));

                    frame.present();
                    window.request_redraw();
                },
                WindowEvent::CloseRequested => {
                    target.exit();
                },
                _ => {}
            }
        }

    }).unwrap();
}