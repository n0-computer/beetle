fn main() {
    prost_build::Config::new()
        .bytes([".unixfs_pb.Data"])
        .compile_protos(&["src/unixfs.proto"], &["src"])
        .unwrap();
}
