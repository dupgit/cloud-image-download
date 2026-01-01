use unwrap_unreachable::UnwrapUnreachable;

#[test]
#[should_panic]
fn option_should_panic() {
    let v: Option<i32> = None;

    v.unreachable();
}

#[test]
#[should_panic]
fn result_should_panic() {
    let v: Result<(), i32> = Err(727);

    v.unreachable();
}

#[test]
fn should_return_ok() {
    let v: Result<String, i32> = Ok("hey man".into());

    assert_eq!(v.unreachable(), "hey man".to_string());
}

#[test]
fn should_return_some() {
    let v: Option<String> = Some("hey man".into());

    assert_eq!(v.unreachable(), "hey man".to_string());
}

#[test]
fn should_chain() {
    let v: Option<Option<Result<String, ()>>> = Some(Some(Ok("hey man".into())));

    assert_eq!(v.unreachable().unreachable().unreachable(), "hey man".to_string());
}
