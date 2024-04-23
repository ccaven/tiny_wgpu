@group(0) @binding(0)
var<storage, read_write> my_buffer: array<u32>;

@compute
@workgroup_size(16, 1, 1)
fn compute(
    @builtin(global_invocation_id) global_id: vec3u
) {
    my_buffer[global_id.x] = my_buffer[global_id.x] * 2u;
}
