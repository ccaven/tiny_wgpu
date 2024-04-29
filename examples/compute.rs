use wgpu::BufferUsages;
use pollster::FutureExt;

use tiny_wgpu::{
    BindGroupItem, Compute, ComputeKernel, ComputeProgram
};

struct ComputeExample<'a> {
    storage: tiny_wgpu::Storage<'a>,
    compute: tiny_wgpu::Compute
}

impl<'a> ComputeProgram<'a> for ComputeExample<'a> {
    fn storage(&self) -> &tiny_wgpu::Storage<'a> {
        &self.storage
    }

    fn storage_mut(&mut self) -> &mut tiny_wgpu::Storage<'a> {
        &mut self.storage
    }

    fn compute(&self) -> &tiny_wgpu::Compute {
        &self.compute
    }
}

fn main() {
    let compute = Compute::new(
        wgpu::Features::empty(),
        wgpu::Limits::default()
    ).block_on();
    
    let storage = Default::default();
    let mut program = ComputeExample { compute, storage };

    program.add_module("compute", wgpu::include_wgsl!("compute.wgsl"));

    program.add_buffer(
        "example_buffer", 
        BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
        128 * 4
    );

    // To send the data back to the CPU
    program.add_staging_buffer("example_buffer");

    program.add_bind_group("example_bind_group", &[
        BindGroupItem::StorageBuffer { label: "example_buffer", min_binding_size: 4, read_only: false }
    ]);

    {
        let bind_groups = &["example_bind_group"];
        let push_constant_ranges = &[];
        program.add_compute_pipelines("compute", bind_groups, &[ComputeKernel { label: "compute", entry_point: "compute" }], push_constant_ranges, None);
    }
    
    // Write data to GPU
    {
        let data: Vec<u32> = (0u32..128).collect();

        program.compute.queue.write_buffer(
            &program.storage().buffers["example_buffer"], 
            0, 
            bytemuck::cast_slice(&data)
        );
    }

    // Run the compute pass
    let mut encoder = program.compute.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: None
    });

    {
        let mut cpass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: None,
            timestamp_writes: None
        });

        cpass.set_pipeline(&program.storage().compute_pipelines["compute"]);
        cpass.set_bind_group(0, &program.storage().bind_groups["example_bind_group"], &[]);
        cpass.dispatch_workgroups(8, 1, 1);
    }

    // To send data back to the CPU, we need to copy it into a staging buffer
    program.copy_buffer_to_staging(&mut encoder, "example_buffer");

    program.compute.queue.submit(Some(encoder.finish()));

    // Then start to map that data
    program.prepare_staging_buffer("example_buffer");

    // Wait for GPU to finish
    program.compute.device.poll(wgpu::Maintain::Wait);

    // Collect output data
    let mut output_destination = vec![0u8; 128 * 4];

    program.read_staging_buffer(
        "example_buffer", 
        &mut output_destination
    );

    let output: &[u32] = bytemuck::cast_slice(&output_destination);

    for i in 0..128 {
        print!("{} ", output[i]);

        assert_eq!(output[i], (i as u32) * 2);
    }
}