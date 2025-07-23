pub mod bindings {
    wit_bindgen::generate!({world: "edge-function", path: ".edgee/wit", generate_all});
    export!(Component);
    pub struct Component;
}
