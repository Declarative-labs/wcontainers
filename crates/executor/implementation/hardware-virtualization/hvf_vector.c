// Sets a vector register of a virtual CPU. The framework takes the value in a vector register, as C passes a
// 16-byte vector, which Rust's stable foreign calls cannot pass.
#include <Hypervisor/Hypervisor.h>
#include <string.h>

hv_return_t wcontainers_set_vector(hv_vcpu_t vcpu, hv_simd_fp_reg_t reg, const void *value) {
    hv_simd_fp_uchar16_t v;
    memcpy(&v, value, sizeof v);
    return hv_vcpu_set_simd_fp_reg(vcpu, reg, v);
}
