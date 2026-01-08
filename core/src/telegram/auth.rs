use async_trait::async_trait;
use grammers_client::types::{LoginToken, PasswordToken};
use grammers_client::{Client, SignInError};
use grammers_tl_types as tl;

use crate::telegram::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthResult<P> {
    Authorized,
    PasswordRequired(P),
    InvalidCode,
    InvalidPassword,
    SignUpRequired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhoneLogin<T> {
    token: T,
}

impl<T> PhoneLogin<T> {
    pub fn token(&self) -> &T {
        &self.token
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QrLogin {
    pub token: Vec<u8>,
    pub expires: Option<i32>,
    pub dc_id: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrLoginResult {
    Pending(QrLogin),
    Authorized,
}

#[async_trait]
pub trait AuthClient: Send + Sync {
    type LoginToken: Send + Sync;
    type PasswordToken: Send + Sync;

    async fn is_authorized(&self) -> Result<bool>;
    async fn request_login_code(&self, phone: &str, api_hash: &str) -> Result<Self::LoginToken>;
    async fn sign_in(
        &self,
        token: &Self::LoginToken,
        code: &str,
    ) -> Result<AuthResult<Self::PasswordToken>>;
    async fn check_password(
        &self,
        token: Self::PasswordToken,
        password: &str,
    ) -> Result<AuthResult<Self::PasswordToken>>;
    async fn export_login_token(
        &self,
        api_id: i32,
        api_hash: &str,
        except_ids: &[i64],
    ) -> Result<QrLoginResult>;
    async fn import_login_token(&self, token: &[u8], dc_id: Option<i32>) -> Result<QrLoginResult>;
}

pub struct AuthFlow<C: AuthClient> {
    client: C,
    api_id: i32,
    api_hash: String,
    except_ids: Vec<i64>,
}

impl<C: AuthClient> AuthFlow<C> {
    pub fn new(client: C, api_id: i32, api_hash: impl Into<String>, except_ids: Vec<i64>) -> Self {
        Self {
            client,
            api_id,
            api_hash: api_hash.into(),
            except_ids,
        }
    }

    pub async fn is_authorized(&self) -> Result<bool> {
        self.client.is_authorized().await
    }

    pub async fn begin_phone_login(&self, phone: &str) -> Result<PhoneLogin<C::LoginToken>> {
        let token = self
            .client
            .request_login_code(phone, &self.api_hash)
            .await?;
        Ok(PhoneLogin { token })
    }

    pub async fn submit_phone_code(
        &self,
        login: &PhoneLogin<C::LoginToken>,
        code: &str,
    ) -> Result<AuthResult<C::PasswordToken>> {
        self.client.sign_in(&login.token, code).await
    }

    pub async fn submit_password(
        &self,
        token: C::PasswordToken,
        password: &str,
    ) -> Result<AuthResult<C::PasswordToken>> {
        self.client.check_password(token, password).await
    }

    pub async fn begin_qr_login(&self) -> Result<QrLoginResult> {
        self.client
            .export_login_token(self.api_id, &self.api_hash, &self.except_ids)
            .await
    }

    pub async fn poll_qr_login(&self, login: &QrLogin) -> Result<QrLoginResult> {
        self.client
            .import_login_token(&login.token, login.dc_id)
            .await
    }
}

pub struct GrammersAuthClient {
    client: Client,
}

impl GrammersAuthClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }

    fn map_sign_in_result(
        &self,
        result: std::result::Result<grammers_client::types::User, SignInError>,
    ) -> Result<AuthResult<PasswordToken>> {
        map_sign_in_result(result)
    }

    fn map_login_token_result(result: tl::enums::auth::LoginToken) -> QrLoginResult {
        map_login_token_result(result)
    }
}

fn map_sign_in_result(
    result: std::result::Result<grammers_client::types::User, SignInError>,
) -> Result<AuthResult<PasswordToken>> {
    match result {
        Ok(_) => Ok(AuthResult::Authorized),
        Err(SignInError::PasswordRequired(token)) => Ok(AuthResult::PasswordRequired(token)),
        Err(SignInError::InvalidCode) => Ok(AuthResult::InvalidCode),
        Err(SignInError::InvalidPassword) => Ok(AuthResult::InvalidPassword),
        Err(SignInError::SignUpRequired { .. }) => Ok(AuthResult::SignUpRequired),
        Err(SignInError::Other(err)) => Err(err.into()),
    }
}

fn map_login_token_result(result: tl::enums::auth::LoginToken) -> QrLoginResult {
    match result {
        tl::enums::auth::LoginToken::Token(token) => QrLoginResult::Pending(QrLogin {
            token: token.token,
            expires: Some(token.expires),
            dc_id: None,
        }),
        tl::enums::auth::LoginToken::MigrateTo(token) => QrLoginResult::Pending(QrLogin {
            token: token.token,
            expires: None,
            dc_id: Some(token.dc_id),
        }),
        tl::enums::auth::LoginToken::Success(_) => QrLoginResult::Authorized,
    }
}

#[cfg(feature = "test-support")]
pub mod test_support {
    use super::*;

    pub fn map_sign_in_result(
        result: std::result::Result<grammers_client::types::User, SignInError>,
    ) -> Result<AuthResult<PasswordToken>> {
        super::map_sign_in_result(result)
    }

    pub fn map_login_token_result(result: tl::enums::auth::LoginToken) -> QrLoginResult {
        super::map_login_token_result(result)
    }
}

#[async_trait]
impl AuthClient for GrammersAuthClient {
    type LoginToken = LoginToken;
    type PasswordToken = PasswordToken;

    async fn is_authorized(&self) -> Result<bool> {
        Ok(self.client.is_authorized().await?)
    }

    async fn request_login_code(&self, phone: &str, api_hash: &str) -> Result<Self::LoginToken> {
        Ok(self.client.request_login_code(phone, api_hash).await?)
    }

    async fn sign_in(
        &self,
        token: &Self::LoginToken,
        code: &str,
    ) -> Result<AuthResult<Self::PasswordToken>> {
        self.map_sign_in_result(self.client.sign_in(token, code).await)
    }

    async fn check_password(
        &self,
        token: Self::PasswordToken,
        password: &str,
    ) -> Result<AuthResult<Self::PasswordToken>> {
        self.map_sign_in_result(self.client.check_password(token, password).await)
    }

    async fn export_login_token(
        &self,
        api_id: i32,
        api_hash: &str,
        except_ids: &[i64],
    ) -> Result<QrLoginResult> {
        let request = tl::functions::auth::ExportLoginToken {
            api_id,
            api_hash: api_hash.to_string(),
            except_ids: except_ids.to_vec(),
        };
        let result = self.client.invoke(&request).await?;
        Ok(Self::map_login_token_result(result))
    }

    async fn import_login_token(&self, token: &[u8], dc_id: Option<i32>) -> Result<QrLoginResult> {
        let request = tl::functions::auth::ImportLoginToken {
            token: token.to_vec(),
        };
        let result = match dc_id {
            Some(dc_id) => self.client.invoke_in_dc(dc_id, &request).await?,
            None => self.client.invoke(&request).await?,
        };
        Ok(Self::map_login_token_result(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    #[derive(Debug, Clone)]
    struct MockAuthClient {
        state: Arc<Mutex<MockState>>,
    }

    #[derive(Debug, Clone)]
    struct MockState {
        authorized: bool,
        login_token: String,
        sign_in_result: AuthResult<String>,
        password_result: AuthResult<String>,
        qr_export_result: QrLoginResult,
        qr_import_results: VecDeque<QrLoginResult>,
    }

    impl MockAuthClient {
        fn new() -> Self {
            Self {
                state: Arc::new(Mutex::new(MockState {
                    authorized: false,
                    login_token: "token".to_string(),
                    sign_in_result: AuthResult::Authorized,
                    password_result: AuthResult::Authorized,
                    qr_export_result: QrLoginResult::Authorized,
                    qr_import_results: VecDeque::new(),
                })),
            }
        }

        fn set_sign_in_result(&self, result: AuthResult<String>) {
            self.state.lock().unwrap().sign_in_result = result;
        }

        fn set_qr_export_result(&self, result: QrLoginResult) {
            self.state.lock().unwrap().qr_export_result = result;
        }

        fn set_qr_import_results(&self, results: Vec<QrLoginResult>) {
            self.state.lock().unwrap().qr_import_results = results.into();
        }
    }

    #[async_trait]
    impl AuthClient for MockAuthClient {
        type LoginToken = String;
        type PasswordToken = String;

        async fn is_authorized(&self) -> Result<bool> {
            Ok(self.state.lock().unwrap().authorized)
        }

        async fn request_login_code(&self, _phone: &str, _api_hash: &str) -> Result<String> {
            Ok(self.state.lock().unwrap().login_token.clone())
        }

        async fn sign_in(&self, _token: &String, _code: &str) -> Result<AuthResult<String>> {
            Ok(self.state.lock().unwrap().sign_in_result.clone())
        }

        async fn check_password(
            &self,
            _token: String,
            _password: &str,
        ) -> Result<AuthResult<String>> {
            Ok(self.state.lock().unwrap().password_result.clone())
        }

        async fn export_login_token(
            &self,
            _api_id: i32,
            _api_hash: &str,
            _except_ids: &[i64],
        ) -> Result<QrLoginResult> {
            Ok(self.state.lock().unwrap().qr_export_result.clone())
        }

        async fn import_login_token(
            &self,
            _token: &[u8],
            _dc_id: Option<i32>,
        ) -> Result<QrLoginResult> {
            let mut state = self.state.lock().unwrap();
            Ok(state
                .qr_import_results
                .pop_front()
                .unwrap_or(QrLoginResult::Authorized))
        }
    }

    #[tokio::test]
    async fn phone_login_returns_token() {
        let client = MockAuthClient::new();
        let flow = AuthFlow::new(client, 1, "hash", vec![]);

        let login = flow.begin_phone_login("+123").await.unwrap();
        assert_eq!(login.token(), "token");
    }

    #[tokio::test]
    async fn phone_login_handles_password_requirement() {
        let client = MockAuthClient::new();
        client.set_sign_in_result(AuthResult::PasswordRequired("pwd".to_string()));
        let flow = AuthFlow::new(client, 1, "hash", vec![]);

        let login = flow.begin_phone_login("+123").await.unwrap();
        let result = flow.submit_phone_code(&login, "12345").await.unwrap();
        assert_eq!(result, AuthResult::PasswordRequired("pwd".to_string()));
    }

    #[tokio::test]
    async fn qr_login_exports_and_polls() {
        let client = MockAuthClient::new();
        client.set_qr_export_result(QrLoginResult::Pending(QrLogin {
            token: vec![1, 2, 3],
            expires: Some(10),
            dc_id: None,
        }));
        client.set_qr_import_results(vec![QrLoginResult::Authorized]);

        let flow = AuthFlow::new(client, 1, "hash", vec![]);
        let result = flow.begin_qr_login().await.unwrap();
        let login = match result {
            QrLoginResult::Pending(login) => login,
            QrLoginResult::Authorized => panic!("expected pending login"),
        };

        let poll = flow.poll_qr_login(&login).await.unwrap();
        assert_eq!(poll, QrLoginResult::Authorized);
    }
}
