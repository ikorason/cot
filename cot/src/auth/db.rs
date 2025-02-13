//! Database-backed user authentication backend.
//!
//! This module provides a user type and an authentication backend that stores
//! the user data in a database using the Cot ORM.

use std::any::Any;
use std::borrow::Cow;

use async_trait::async_trait;
use cot::form::{FormContext, FormResult};
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha512;
use thiserror::Error;

use crate::admin::{AdminModel, AdminModelManager, DefaultAdminModelManager};
use crate::auth::{
    AuthBackend, AuthError, Password, PasswordHash, PasswordVerificationResult, Result,
    SessionAuthHash, User, UserId,
};
use crate::config::SecretKey;
use crate::db::migrations::SyncDynMigration;
use crate::db::{model, query, Auto, DatabaseBackend, LimitedString, Model};
use crate::form::Form;
use crate::request::{Request, RequestExt};
use crate::App;

pub mod migrations;

pub(crate) const MAX_USERNAME_LENGTH: u32 = 255;

/// A user stored in the database.
#[derive(Debug, Clone, Form)]
#[model]
pub struct DatabaseUser {
    id: Auto<i64>,
    #[model(unique)]
    username: LimitedString<MAX_USERNAME_LENGTH>,
    password: PasswordHash,
}

/// An error that occurs when creating a user.
#[derive(Debug, Clone, Error)]
#[non_exhaustive]
pub enum CreateUserError {
    /// The username is too long.
    #[error("username is too long (max {MAX_USERNAME_LENGTH} characters, got {0})")]
    UsernameTooLong(usize),
}

impl DatabaseUser {
    #[must_use]
    fn new(
        id: Auto<i64>,
        username: LimitedString<MAX_USERNAME_LENGTH>,
        password: &Password,
    ) -> Self {
        Self {
            id,
            username,
            password: PasswordHash::from_password(password),
        }
    }

    /// Create a new user and save it to the database.
    ///
    /// # Errors
    ///
    /// Returns an error if the user could not be saved to the database.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUser;
    /// use cot::auth::Password;
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::{Response, ResponseExt};
    /// use cot::{Body, StatusCode};
    ///
    /// async fn view(request: &Request) -> cot::Result<Response> {
    ///     let user = DatabaseUser::create_user(
    ///         request.db(),
    ///         "testuser".to_string(),
    ///         &Password::new("password123"),
    ///     )
    ///     .await?;
    ///
    ///     Ok(Response::new_html(
    ///         StatusCode::OK,
    ///         Body::fixed("User created!"),
    ///     ))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// #     use cot::test::{TestDatabase, TestRequestBuilder};
    /// #     let mut test_database = TestDatabase::new_sqlite().await?;
    /// #     test_database.with_auth().run_migrations().await;
    /// #     let request = TestRequestBuilder::get("/")
    /// #         .with_db_auth(test_database.database())
    /// #         .build();
    /// #     view(&request).await?;
    /// #     test_database.cleanup().await?;
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn create_user<DB: DatabaseBackend, T: Into<String>, U: Into<Password>>(
        db: &DB,
        username: T,
        password: U,
    ) -> Result<Self> {
        let username = username.into();
        let username_length = username.len();
        let username = LimitedString::<MAX_USERNAME_LENGTH>::new(username).map_err(|_| {
            AuthError::backend_error(CreateUserError::UsernameTooLong(username_length))
        })?;

        let mut user = Self::new(Auto::auto(), username, &password.into());
        user.insert(db).await.map_err(AuthError::backend_error)?;

        Ok(user)
    }

    /// Get a user by their integer ID. Returns [`None`] if the user does not
    /// exist.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an error querying the database.
    ///
    /// # Panics
    ///
    /// Panics if the user ID provided is not an integer.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUser;
    /// use cot::auth::{Password, UserId};
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::{Response, ResponseExt};
    /// use cot::{Body, StatusCode};
    ///
    /// async fn view(request: &Request) -> cot::Result<Response> {
    ///     let user = DatabaseUser::create_user(
    ///         request.db(),
    ///         "testuser".to_string(),
    ///         &Password::new("password123"),
    ///     )
    ///     .await?;
    ///
    ///     let user_from_db = DatabaseUser::get_by_id(request.db(), user.id()).await?;
    ///
    ///     Ok(Response::new_html(
    ///         StatusCode::OK,
    ///         Body::fixed("User created!"),
    ///     ))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// #     use cot::test::{TestDatabase, TestRequestBuilder};
    /// #     let mut test_database = TestDatabase::new_sqlite().await?;
    /// #     test_database.with_auth().run_migrations().await;
    /// #     let request = TestRequestBuilder::get("/")
    /// #         .with_db_auth(test_database.database())
    /// #         .build();
    /// #     view(&request).await?;
    /// #     test_database.cleanup().await?;
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn get_by_id<DB: DatabaseBackend>(db: &DB, id: i64) -> Result<Option<Self>> {
        let db_user = query!(DatabaseUser, $id == id)
            .get(db)
            .await
            .map_err(AuthError::backend_error)?;

        Ok(db_user)
    }

    /// Get a user by their username. Returns [`None`] if the user does not
    /// exist.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an error querying the database.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUser;
    /// use cot::auth::{Password, UserId};
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::{Response, ResponseExt};
    /// use cot::{Body, StatusCode};
    ///
    /// async fn view(request: &Request) -> cot::Result<Response> {
    ///     let user = DatabaseUser::create_user(
    ///         request.db(),
    ///         "testuser".to_string(),
    ///         &Password::new("password123"),
    ///     )
    ///     .await?;
    ///
    ///     let user_from_db = DatabaseUser::get_by_username(request.db(), "testuser").await?;
    ///
    ///     Ok(Response::new_html(
    ///         StatusCode::OK,
    ///         Body::fixed("User created!"),
    ///     ))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// #     use cot::test::{TestDatabase, TestRequestBuilder};
    /// #     let mut test_database = TestDatabase::new_sqlite().await?;
    /// #     test_database.with_auth().run_migrations().await;
    /// #     let request = TestRequestBuilder::get("/")
    /// #         .with_db_auth(test_database.database())
    /// #         .build();
    /// #     view(&request).await?;
    /// #     test_database.cleanup().await?;
    /// #     Ok(())
    /// # }
    /// ```
    pub async fn get_by_username<DB: DatabaseBackend>(
        db: &DB,
        username: &str,
    ) -> Result<Option<Self>> {
        let username = LimitedString::<MAX_USERNAME_LENGTH>::new(username).map_err(|_| {
            AuthError::backend_error(CreateUserError::UsernameTooLong(username.len()))
        })?;
        let db_user = query!(DatabaseUser, $username == username)
            .get(db)
            .await
            .map_err(AuthError::backend_error)?;

        Ok(db_user)
    }

    /// Authenticate a user.
    ///
    /// # Errors
    ///
    /// Returns an error if there was an error querying the database.
    pub async fn authenticate<DB: DatabaseBackend>(
        db: &DB,
        credentials: &DatabaseUserCredentials,
    ) -> Result<Option<Self>> {
        let username = credentials.username();
        let username_limited = LimitedString::<MAX_USERNAME_LENGTH>::new(username.to_string())
            .map_err(|_| {
                AuthError::backend_error(CreateUserError::UsernameTooLong(username.len()))
            })?;
        let user = query!(DatabaseUser, $username == username_limited)
            .get(db)
            .await
            .map_err(AuthError::backend_error)?;

        if let Some(mut user) = user {
            let password_hash = &user.password;
            match password_hash.verify(credentials.password()) {
                PasswordVerificationResult::Ok => Ok(Some(user)),
                PasswordVerificationResult::OkObsolete(new_hash) => {
                    user.password = new_hash;
                    user.save(db).await.map_err(AuthError::backend_error)?;
                    Ok(Some(user))
                }
                PasswordVerificationResult::Invalid => Ok(None),
            }
        } else {
            // SECURITY: If no user was found, run the same hashing function to prevent
            // timing attacks from being used to determine if a user exists. Additionally,
            // do something with the result to prevent the compiler from optimizing out the
            // operation.
            // TODO: benchmark this to make sure it works as expected
            let dummy_hash = PasswordHash::from_password(credentials.password());
            if let PasswordVerificationResult::Invalid = dummy_hash.verify(credentials.password()) {
                unreachable!(
                    "Password hash verification should never fail for a newly generated hash"
                );
            }
            Ok(None)
        }
    }

    /// Get the ID of the user.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUser;
    /// use cot::auth::{Password, UserId};
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::{Response, ResponseExt};
    /// use cot::{Body, StatusCode};
    ///
    /// async fn view(request: &Request) -> cot::Result<Response> {
    ///     let user = DatabaseUser::create_user(
    ///         request.db(),
    ///         "testuser".to_string(),
    ///         &Password::new("password123"),
    ///     )
    ///     .await?;
    ///
    ///     Ok(Response::new_html(
    ///         StatusCode::OK,
    ///         Body::fixed(format!("User ID: {}", user.id())),
    ///     ))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// #     use cot::test::{TestDatabase, TestRequestBuilder};
    /// #     let mut test_database = TestDatabase::new_sqlite().await?;
    /// #     test_database.with_auth().run_migrations().await;
    /// #     let request = TestRequestBuilder::get("/")
    /// #         .with_db_auth(test_database.database())
    /// #         .build();
    /// #     view(&request).await?;
    /// #     test_database.cleanup().await?;
    /// #     Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn id(&self) -> i64 {
        match self.id {
            Auto::Fixed(id) => id,
            Auto::Auto => unreachable!("DatabaseUser constructed with an unknown ID"),
        }
    }

    /// Get the username of the user.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUser;
    /// use cot::auth::{Password, UserId};
    /// use cot::request::{Request, RequestExt};
    /// use cot::response::{Response, ResponseExt};
    /// use cot::{Body, StatusCode};
    ///
    /// async fn view(request: &Request) -> cot::Result<Response> {
    ///     let user = DatabaseUser::create_user(
    ///         request.db(),
    ///         "testuser".to_string(),
    ///         &Password::new("password123"),
    ///     )
    ///     .await?;
    ///
    ///     Ok(Response::new_html(
    ///         StatusCode::OK,
    ///         Body::fixed(user.username().to_string()),
    ///     ))
    /// }
    ///
    /// # #[tokio::main]
    /// # async fn main() -> cot::Result<()> {
    /// #     use cot::test::{TestDatabase, TestRequestBuilder};
    /// #     let mut test_database = TestDatabase::new_sqlite().await?;
    /// #     test_database.with_auth().run_migrations().await;
    /// #     let request = TestRequestBuilder::get("/")
    /// #         .with_db_auth(test_database.database())
    /// #         .build();
    /// #     view(&request).await?;
    /// #     test_database.cleanup().await?;
    /// #     Ok(())
    /// # }
    /// ```
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

type SessionAuthHmac = Hmac<Sha512>;

impl User for DatabaseUser {
    fn id(&self) -> Option<UserId> {
        Some(UserId::Int(self.id()))
    }

    fn username(&self) -> Option<Cow<'_, str>> {
        Some(Cow::from(self.username.as_str()))
    }

    fn is_active(&self) -> bool {
        true
    }

    fn is_authenticated(&self) -> bool {
        true
    }

    fn session_auth_hash(&self, secret_key: &SecretKey) -> Option<SessionAuthHash> {
        let mut mac = SessionAuthHmac::new_from_slice(secret_key.as_bytes())
            .expect("HMAC can take key of any size");
        mac.update(self.password.as_str().as_bytes());
        let hmac_data = mac.finalize().into_bytes();

        Some(SessionAuthHash::new(&hmac_data))
    }
}

#[async_trait]
impl AdminModel for DatabaseUser {
    fn as_any(&self) -> &dyn Any {
        self
    }

    async fn get_objects(request: &Request) -> crate::Result<Vec<Self>> {
        Ok(Self::objects().all(request.db()).await?)
    }

    async fn get_object_by_id(request: &Request, id: &str) -> cot::Result<Option<Self>>
    where
        Self: Sized,
    {
        let id = Self::parse_id(id)?;

        Ok(query!(Self, $id == id).get(request.db()).await?)
    }

    fn name() -> &'static str {
        "Database User"
    }

    fn url_name() -> &'static str {
        "database_user"
    }

    fn id(&self) -> String {
        self.id().to_string()
    }

    fn display(&self) -> String {
        self.username.as_str().to_owned()
    }

    fn form_context() -> Box<dyn FormContext>
    where
        Self: Sized,
    {
        Box::new(<Self as Form>::Context::new())
    }

    fn form_context_from_self(&self) -> Box<dyn FormContext> {
        Box::new(<Self as Form>::to_context(self))
    }

    async fn save_from_request(
        request: &mut Request,
        object_id: Option<&str>,
    ) -> cot::Result<Option<Box<dyn FormContext>>>
    where
        Self: Sized,
    {
        let form_result = <Self as Form>::from_request(request).await?;
        match form_result {
            FormResult::Ok(mut object_from_form) => {
                if let Some(object_id) = object_id {
                    let id = Self::parse_id(object_id)?;

                    object_from_form.set_primary_key(Auto::fixed(id));
                    object_from_form.update(request.db()).await?;
                } else {
                    object_from_form.insert(request.db()).await?;
                }
                Ok(None)
            }
            FormResult::ValidationError(context) => Ok(Some(Box::new(context))),
        }
    }

    async fn remove_by_id(request: &mut Request, object_id: &str) -> cot::Result<()>
    where
        Self: Sized,
    {
        let id = Self::parse_id(object_id)?;

        query!(Self, $id == id).delete(request.db()).await?;

        Ok(())
    }
}

impl DatabaseUser {
    fn parse_id(id: &str) -> cot::Result<i64> {
        id.parse::<i64>()
            .map_err(|_| cot::Error::not_found_message(format!("Invalid DatabaseUser ID: `{id}`")))
    }
}

/// Credentials for authenticating a user stored in the database.
///
/// This struct is used to authenticate a user stored in the database. It
/// contains the username and password of the user.
///
/// Can be passed to
/// [`AuthRequestExt::authenticate`](crate::auth::AuthRequestExt::authenticate)
/// to authenticate a user when using the [`DatabaseUserBackend`].
#[derive(Debug, Clone)]
pub struct DatabaseUserCredentials {
    username: String,
    password: Password,
}

impl DatabaseUserCredentials {
    /// Create a new instance of the database user credentials.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUserCredentials;
    /// use cot::auth::Password;
    ///
    /// let credentials =
    ///     DatabaseUserCredentials::new(String::from("testuser"), Password::new("password123"));
    /// ```
    #[must_use]
    pub fn new(username: String, password: Password) -> Self {
        Self { username, password }
    }

    /// Get the username of the user.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUserCredentials;
    /// use cot::auth::Password;
    ///
    /// let credentials =
    ///     DatabaseUserCredentials::new(String::from("testuser"), Password::new("password123"));
    /// assert_eq!(credentials.username(), "testuser");
    /// ```
    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Get the password of the user.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUserCredentials;
    /// use cot::auth::Password;
    ///
    /// let credentials =
    ///     DatabaseUserCredentials::new(String::from("testuser"), Password::new("password123"));
    /// assert!(!credentials.password().as_str().is_empty());
    /// ```
    #[must_use]
    pub fn password(&self) -> &Password {
        &self.password
    }
}

/// The authentication backend for users stored in the database.
///
/// This is the default authentication backend for Cot. It authenticates
/// users stored in the database using the [`DatabaseUser`] model.
///
/// This backend supports authenticating users using the
/// [`DatabaseUserCredentials`] struct and ignores all other credential types.
#[derive(Debug, Copy, Clone)]
pub struct DatabaseUserBackend;

impl Default for DatabaseUserBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseUserBackend {
    /// Create a new instance of the database user authentication backend.
    ///
    /// # Example
    ///
    /// ```
    /// use cot::auth::db::DatabaseUserBackend;
    /// use cot::auth::AuthBackend;
    /// use cot::config::ProjectConfig;
    /// use cot::project::WithApps;
    /// use cot::{Project, ProjectContext};
    ///
    /// struct HelloProject;
    /// impl Project for HelloProject {
    ///     fn auth_backend(&self, app_context: &ProjectContext<WithApps>) -> Box<dyn AuthBackend> {
    ///         Box::new(DatabaseUserBackend::new())
    ///         // note that it's usually better to just set the auth backend in the config
    ///     }
    /// }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl AuthBackend for DatabaseUserBackend {
    async fn authenticate(
        &self,
        request: &Request,
        credentials: &(dyn Any + Send + Sync),
    ) -> Result<Option<Box<dyn User + Send + Sync>>> {
        if let Some(credentials) = credentials.downcast_ref::<DatabaseUserCredentials>() {
            #[allow(trivial_casts)] // Upcast to the correct Box type
            Ok(DatabaseUser::authenticate(request.db(), credentials)
                .await
                .map(|user| user.map(|user| Box::new(user) as Box<dyn User + Send + Sync>))?)
        } else {
            Err(AuthError::CredentialsTypeNotSupported)
        }
    }

    async fn get_by_id(
        &self,
        request: &Request,
        id: UserId,
    ) -> Result<Option<Box<dyn User + Send + Sync>>> {
        let UserId::Int(id) = id else {
            return Err(AuthError::UserIdTypeNotSupported);
        };

        #[allow(trivial_casts)] // Upcast to the correct Box type
        Ok(DatabaseUser::get_by_id(request.db(), id)
            .await?
            .map(|user| Box::new(user) as Box<dyn User + Send + Sync>))
    }
}

#[derive(Debug, Copy, Clone)]
pub struct DatabaseUserApp;

impl Default for DatabaseUserApp {
    fn default() -> Self {
        Self::new()
    }
}

impl DatabaseUserApp {
    /// Create a new instance of the database user authentication app.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use cot::auth::db::DatabaseUserApp;
    /// use cot::config::{DatabaseConfig, ProjectConfig};
    /// use cot::project::WithConfig;
    /// use cot::{App, AppBuilder, Project, ProjectContext};
    ///
    /// struct HelloProject;
    /// impl Project for HelloProject {
    ///     fn config(&self, config_name: &str) -> cot::Result<ProjectConfig> {
    ///         Ok(ProjectConfig::builder()
    ///             .database(DatabaseConfig::builder().url("sqlite::memory:").build())
    ///             .build())
    ///     }
    ///
    ///     fn register_apps(&self, apps: &mut AppBuilder, _context: &ProjectContext<WithConfig>) {
    ///         use cot::project::WithConfig;
    ///         apps.register_with_views(DatabaseUserApp::new(), "");
    ///     }
    /// }
    ///
    /// #[cot::main]
    /// fn main() -> impl Project {
    ///     HelloProject
    /// }
    /// ```
    #[must_use]
    pub fn new() -> Self {
        Self {}
    }
}

impl App for DatabaseUserApp {
    fn name(&self) -> &'static str {
        "cot_db_user"
    }

    fn admin_model_managers(&self) -> Vec<Box<dyn AdminModelManager>> {
        vec![Box::new(DefaultAdminModelManager::<DatabaseUser>::new())]
    }

    fn migrations(&self) -> Vec<Box<SyncDynMigration>> {
        // TODO: this is way too complicated for the user-facing API
        #[allow(trivial_casts)]
        migrations::MIGRATIONS
            .iter()
            .copied()
            .map(|x| Box::new(x) as Box<SyncDynMigration>)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SecretKey;
    use crate::db::MockDatabaseBackend;

    #[test]
    #[cfg_attr(miri, ignore)]
    fn session_auth_hash() {
        let user = DatabaseUser::new(
            Auto::fixed(1),
            LimitedString::new("testuser").unwrap(),
            &Password::new("password123"),
        );
        let secret_key = SecretKey::new(b"supersecretkey");

        let hash = user.session_auth_hash(&secret_key);
        assert!(hash.is_some());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn database_user_traits() {
        let user = DatabaseUser::new(
            Auto::fixed(1),
            LimitedString::new("testuser").unwrap(),
            &Password::new("password123"),
        );
        let user_ref: &dyn User = &user;
        assert_eq!(user_ref.id(), Some(UserId::Int(1)));
        assert_eq!(user_ref.username(), Some(Cow::from("testuser")));
        assert!(user_ref.is_active());
        assert!(user_ref.is_authenticated());
        assert!(user_ref
            .session_auth_hash(&SecretKey::new(b"supersecretkey"))
            .is_some());
    }

    #[cot::test]
    #[cfg_attr(miri, ignore)]
    async fn create_user() {
        let mut mock_db = MockDatabaseBackend::new();
        mock_db
            .expect_insert::<DatabaseUser>()
            .returning(|_| Ok(()));

        let username = "testuser".to_string();
        let password = Password::new("password123");

        let user = DatabaseUser::create_user(&mock_db, username.clone(), &password)
            .await
            .unwrap();
        assert_eq!(user.username(), username);
    }

    #[cot::test]
    #[cfg_attr(miri, ignore)]
    async fn get_by_id() {
        let mut mock_db = MockDatabaseBackend::new();
        let user = DatabaseUser::new(
            Auto::fixed(1),
            LimitedString::new("testuser").unwrap(),
            &Password::new("password123"),
        );

        mock_db
            .expect_get::<DatabaseUser>()
            .returning(move |_| Ok(Some(user.clone())));

        let result = DatabaseUser::get_by_id(&mock_db, 1).await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().username(), "testuser");
    }

    #[cot::test]
    #[cfg_attr(miri, ignore)]
    async fn authenticate() {
        let mut mock_db = MockDatabaseBackend::new();
        let user = DatabaseUser::new(
            Auto::fixed(1),
            LimitedString::new("testuser").unwrap(),
            &Password::new("password123"),
        );

        mock_db
            .expect_get::<DatabaseUser>()
            .returning(move |_| Ok(Some(user.clone())));

        let credentials =
            DatabaseUserCredentials::new("testuser".to_string(), Password::new("password123"));
        let result = DatabaseUser::authenticate(&mock_db, &credentials)
            .await
            .unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap().username(), "testuser");
    }

    #[cot::test]
    #[cfg_attr(miri, ignore)]
    async fn authenticate_non_existing() {
        let mut mock_db = MockDatabaseBackend::new();

        mock_db
            .expect_get::<DatabaseUser>()
            .returning(move |_| Ok(None));

        let credentials =
            DatabaseUserCredentials::new("testuser".to_string(), Password::new("password123"));
        let result = DatabaseUser::authenticate(&mock_db, &credentials)
            .await
            .unwrap();
        assert!(result.is_none());
    }

    #[cot::test]
    #[cfg_attr(miri, ignore)]
    async fn authenticate_invalid_password() {
        let mut mock_db = MockDatabaseBackend::new();
        let user = DatabaseUser::new(
            Auto::fixed(1),
            LimitedString::new("testuser").unwrap(),
            &Password::new("password123"),
        );

        mock_db
            .expect_get::<DatabaseUser>()
            .returning(move |_| Ok(Some(user.clone())));

        let credentials =
            DatabaseUserCredentials::new("testuser".to_string(), Password::new("invalid"));
        let result = DatabaseUser::authenticate(&mock_db, &credentials)
            .await
            .unwrap();
        assert!(result.is_none());
    }
}
