use grammers_client::types::{PasswordToken, User as GrammersUser};
use grammers_client::SignInError;
use grammers_mtsender::InvocationError;
use grammers_tl_types as tl;
use telegram_llm_core::telegram::auth::test_support::{map_login_token_result, map_sign_in_result};
use telegram_llm_core::telegram::{AuthResult, QrLogin, QrLoginResult, TelegramError};

fn sample_password_token() -> PasswordToken {
    let password = tl::types::account::Password {
        has_recovery: false,
        has_secure_values: false,
        has_password: true,
        current_algo: None,
        srp_b: None,
        srp_id: None,
        hint: None,
        email_unconfirmed_pattern: None,
        new_algo: tl::enums::PasswordKdfAlgo::Unknown,
        new_secure_algo: tl::enums::SecurePasswordKdfAlgo::Unknown,
        secure_random: Vec::new(),
        pending_reset_date: None,
        login_email_pattern: None,
    };

    PasswordToken::new(password)
}

fn sample_login_token_success() -> tl::types::auth::LoginTokenSuccess {
    let auth = tl::types::auth::Authorization {
        setup_password_required: false,
        otherwise_relogin_days: None,
        tmp_sessions: None,
        future_auth_token: None,
        user: tl::enums::User::Empty(tl::types::UserEmpty { id: 1 }),
    };

    tl::types::auth::LoginTokenSuccess {
        authorization: tl::enums::auth::Authorization::Authorization(auth),
    }
}

#[test]
fn sign_in_maps_authorized() {
    let raw_user = tl::enums::User::Empty(tl::types::UserEmpty { id: 42 });
    let user = GrammersUser::from_raw(raw_user);

    let result = map_sign_in_result(Ok(user)).unwrap();
    assert!(matches!(result, AuthResult::Authorized));
}

#[test]
fn sign_in_maps_password_required() {
    let token = sample_password_token();
    let result = map_sign_in_result(Err(SignInError::PasswordRequired(token))).unwrap();

    match result {
        AuthResult::PasswordRequired(token) => {
            assert!(token.hint().is_none());
        }
        _ => panic!("expected password required"),
    }
}

#[test]
fn sign_in_maps_invalid_code() {
    let result = map_sign_in_result(Err(SignInError::InvalidCode)).unwrap();
    assert!(matches!(result, AuthResult::InvalidCode));
}

#[test]
fn sign_in_maps_invalid_password() {
    let result = map_sign_in_result(Err(SignInError::InvalidPassword)).unwrap();
    assert!(matches!(result, AuthResult::InvalidPassword));
}

#[test]
fn sign_in_maps_sign_up_required() {
    let result = map_sign_in_result(Err(SignInError::SignUpRequired {
        terms_of_service: None,
    }))
    .unwrap();

    assert!(matches!(result, AuthResult::SignUpRequired));
}

#[test]
fn sign_in_maps_other_to_error() {
    let result = map_sign_in_result(Err(SignInError::Other(InvocationError::Dropped)));
    let err = result.expect_err("expected invocation error");
    assert!(matches!(
        err,
        TelegramError::Invocation(InvocationError::Dropped)
    ));
}

#[test]
fn login_token_maps_pending_token() {
    let token = tl::types::auth::LoginToken {
        expires: 120,
        token: vec![1, 2, 3],
    };

    let result = map_login_token_result(tl::enums::auth::LoginToken::Token(token));
    assert_eq!(
        result,
        QrLoginResult::Pending(QrLogin {
            token: vec![1, 2, 3],
            expires: Some(120),
            dc_id: None,
        })
    );
}

#[test]
fn login_token_maps_pending_migrate() {
    let token = tl::types::auth::LoginTokenMigrateTo {
        dc_id: 3,
        token: vec![9, 9, 9],
    };

    let result = map_login_token_result(tl::enums::auth::LoginToken::MigrateTo(token));
    assert_eq!(
        result,
        QrLoginResult::Pending(QrLogin {
            token: vec![9, 9, 9],
            expires: None,
            dc_id: Some(3),
        })
    );
}

#[test]
fn login_token_maps_success() {
    let token = sample_login_token_success();
    let result = map_login_token_result(tl::enums::auth::LoginToken::Success(token));

    assert_eq!(result, QrLoginResult::Authorized);
}
