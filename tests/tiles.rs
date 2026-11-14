use dulook::tiles;

#[test]
fn tile_keys_must_lie_inside_the_map_at_their_zoom() {
    assert!(tiles::valid_key("16/34065/21355"));
    assert!(tiles::valid_key("0/0/0"));
    for key in ["20/0/0", "2/4/0", "2/0/4", "a/b/c", "1/2", "1/0/0/0", "1/-1/0", "1/+1/0", ""] {
        assert!(!tiles::valid_key(key), "{key}");
    }
}
