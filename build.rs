fn main() {
    embed_resource::compile("deskicons.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
