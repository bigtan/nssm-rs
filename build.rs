extern crate winresource;

fn main() {
    if cfg!(target_os = "windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("nssm-rs.ico");
        res.compile().unwrap();
    }
}
