use pollster::FutureExt;
use winit::{event::{Event, WindowEvent}, event_loop::EventLoop, window::Window};
use tiny_wgpu::{Compute, ComputeProgram, RenderKernel, Storage};

struct WindowExample<'a> {
    storage: tiny_wgpu::Storage<'a>,
    compute: tiny_wgpu::Compute,
    surface: wgpu::Surface<'a>
}

impl<'a> ComputeProgram<'a> for WindowExample<'a> {
    fn storage(&self) -> &Storage<'a> {
        &self.storage
    }

    fn storage_mut(&mut self) -> &mut Storage<'a> {
        &mut self.storage
    }

    fn compute(&self) -> &Compute {
        &self.compute
    }
}

fn main() {

    let event_loop = EventLoop::new().unwrap();
    let window = Window::new(&event_loop).unwrap();

    let compute = Compute::new(
        wgpu::Features::empty(),
        wgpu::Limits::default()
    ).block_on();

    let surface = compute.instance.create_surface(&window).unwrap();

    let storage = Default::default();
    
    let mut program = WindowExample { compute, surface, storage };

    program.add_module("window", wgpu::include_wgsl!("window.wgsl"));

    let swapchain_capabilities = program.surface.get_capabilities(&program.compute().adapter);
    let swapchain_format = swapchain_capabilities.formats[0];
    program.add_render_pipelines(
        "window",
         &[],
         &[RenderKernel { label: "window", vertex: "vs_main", fragment: "fs_main" }], 
         &[], 
         &[Some(swapchain_format.into())], 
         &[], 
         None, 
         None
    );

    let mut config = program.surface
        .get_default_config(&program.compute().adapter, 400, 400)
        .unwrap();
    
    program.surface.configure(&program.compute().device, &config);

    let window = &window;

    event_loop.run(move |event, target| {

        if let Event::WindowEvent { window_id: _window_id, event } = event {
            match event {
                WindowEvent::Resized(new_size) => {
                    config.width = new_size.width.max(1);
                    config.height = new_size.height.max(1);
                    program.surface.configure(&program.compute().device, &config);
                    window.request_redraw();
                },
                WindowEvent::RedrawRequested => {
                    let frame = program.surface.get_current_texture().unwrap();
                    let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());

                    let mut encoder = program.compute().device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
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
                        rpass.set_pipeline(&program.storage().render_pipelines["window"]);
                        rpass.draw(0..3, 0..1);
                    }

                    program.compute().queue.submit(Some(encoder.finish()));

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