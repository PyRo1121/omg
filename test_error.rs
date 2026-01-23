use omg_lib::core::safe_ops::nonzero_u32;

fn main() {
    let result = nonzero_u32(0, "test");
    if let Err(e) = result {
        println!("Error: {}", e);
    }
}
