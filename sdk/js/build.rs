// Required by napi-build — registers the Node.js native addon correctly
// on all platforms (handles .node extension, symbol export, etc.).
extern crate napi_build;

fn main() {
    napi_build::setup();
}
