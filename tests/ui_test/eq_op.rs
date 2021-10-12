#[test]
fn eq_op_shouldnt_trigger_in_tests() {
    let a = 1;
    let result = a + 1 == 1 + a;
    assert!(result);
}
