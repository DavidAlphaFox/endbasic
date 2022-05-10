// EndBASIC
// Copyright 2021 Julio Merino
//
// Licensed under the Apache License, Version 2.0 (the "License"); you may not
// use this file except in compliance with the License.  You may obtain a copy
// of the License at:
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS, WITHOUT
// WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.  See the
// License for the specific language governing permissions and limitations
// under the License.

//! Commands to interact with the cloud service.

use crate::*;
use async_trait::async_trait;
use endbasic_core::ast::{ArgSep, Expr, Value, VarType};
use endbasic_core::exec::Machine;
use endbasic_core::syms::{
    CallError, CallableMetadata, CallableMetadataBuilder, Command, CommandResult,
};
use endbasic_std::console::{read_line_secure, refill_and_print, Console};
use endbasic_std::storage::{FileAcls, Storage};
use std::cell::RefCell;
use std::rc::Rc;
use std::str;

/// Category description for all symbols provided by this module.
const CATEGORY: &str = "Cloud access
The EndBASIC service, should you choose to create an account in, is a cloud service that provides \
online file sharing across users of EndBASIC.  The commands below allow you interact with this \
service once you have created an account.
To create an account, visit https://www.endbasic.dev/service.html and come back here once the \
account is ready.
During account creation time, you are assigned a unique, persistent drive in which you can store \
files privately.  You can later choose to share individual files with the public or with specific \
individuals, at which point those people will be able to see them by mounting your drive.
Once logged in, the cloud:// file system scheme becomes available.  You can use it to mount other \
people's drives by specifying their username as the path.  For example, a command like the \
following would mount user-123's shared files as a new drive X: MOUNT \"X\", \"cloud://user-123\"";

/// The `LOGIN` command.
pub struct LoginCommand {
    metadata: CallableMetadata,
    service: Rc<RefCell<dyn Service>>,
    console: Rc<RefCell<dyn Console>>,
    storage: Rc<RefCell<Storage>>,
}

impl LoginCommand {
    /// Creates a new `LOGIN` command.
    pub fn new(
        service: Rc<RefCell<dyn Service>>,
        console: Rc<RefCell<dyn Console>>,
        storage: Rc<RefCell<Storage>>,
    ) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("LOGIN", VarType::Void)
                .with_syntax("username$ [password$]")
                .with_category(CATEGORY)
                .with_description(
                    "Logs into the user's account.
On a successful login, this mounts your personal drive under the CLOUD:/ location, which you can \
access with any other file-related commands.  Using the cloud:// file system scheme, you can mount \
other people's drives with the MOUNT command.
To create an account, visit https://www.endbasic.dev/service.html and come back here once the \
account is ready.",
                )
                .build(),
            service,
            console,
            storage,
        })
    }

    /// Performs the login workflow against the server.
    async fn do_login(&self, username: &str, password: &str) -> CommandResult {
        let response = self.service.borrow_mut().login(username, password).await?;

        {
            let console = &mut *self.console.borrow_mut();
            if !response.motd.is_empty() {
                console.print("")?;
                console.print("----- BEGIN SERVER MOTD -----")?;
                for line in response.motd {
                    refill_and_print(console, &line, "")?;
                }
                console.print("-----  END SERVER MOTD  -----")?;
                console.print("")?;
            }
        }

        let mut storage = self.storage.borrow_mut();
        storage.register_scheme(
            "cloud",
            Box::from(CloudDriveFactory::new(self.service.clone(), response.access_token)),
        );
        storage.mount("CLOUD", &format!("cloud://{}", username))?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl Command for LoginCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, args: &[(Option<Expr>, ArgSep)], machine: &mut Machine) -> CommandResult {
        if self.storage.borrow().has_scheme("cloud") {
            // TODO(jmmv): To support authenticating more than once in one session, we have to
            // either refresh the access tokens of any mounted drive or unmount them all.  Plus we
            // have to avoid re-registering or re-creating the "cloud" scheme.
            return Err(CallError::InternalError(
                "Support for calling LOGIN twice in the same session is not implemented".to_owned(),
            ));
        }

        let (username, password) = match args {
            [(Some(username), ArgSep::End)] => {
                match username.eval(machine.get_mut_symbols()).await? {
                    Value::Text(username) => {
                        let password =
                            read_line_secure(&mut *self.console.borrow_mut(), "Password: ").await?;
                        (username, password)
                    }
                    _ => {
                        return Err(CallError::ArgumentError(
                            "LOGIN requires a string as the username".to_owned(),
                        ))
                    }
                }
            }
            [(Some(username), ArgSep::Long), (Some(password), ArgSep::End)] => {
                let username = match username.eval(machine.get_mut_symbols()).await? {
                    Value::Text(username) => username,
                    _ => {
                        return Err(CallError::ArgumentError(
                            "LOGIN requires a string as the username".to_owned(),
                        ))
                    }
                };
                let password = match password.eval(machine.get_mut_symbols()).await? {
                    Value::Text(password) => password,
                    _ => {
                        return Err(CallError::ArgumentError(
                            "LOGIN requires a string as the password".to_owned(),
                        ))
                    }
                };
                (username, password)
            }
            _ => {
                return Err(CallError::ArgumentError(
                    "LOGIN requires one or two arguments".to_owned(),
                ))
            }
        };

        self.do_login(&username, &password).await
    }
}

/// The `SHARE` command.
///
/// Note that this command is not exclusively for use by the cloud drive as this interacts with the
/// generic storage layer.  As a result, one might say that this command belongs where other disk
/// commands such as `DIR` are defined, but given that ACLs are primarily a cloud concept in our
/// case, it makes sense to keep it here.
pub struct ShareCommand {
    metadata: CallableMetadata,
    console: Rc<RefCell<dyn Console>>,
    storage: Rc<RefCell<Storage>>,
}

impl ShareCommand {
    /// Creates a new `SHARE` command.
    pub fn new(console: Rc<RefCell<dyn Console>>, storage: Rc<RefCell<Storage>>) -> Rc<Self> {
        Rc::from(Self {
            metadata: CallableMetadataBuilder::new("SHARE", VarType::Void)
                .with_syntax("filename$ [, acl1$, .., aclN$]")
                .with_category(CATEGORY)
                .with_description(
                    "Displays or modifies the ACLs of a file.
If given only a filename$, this command prints out the ACLs of the file.
Otherwise, when given a list of ACL changes, applies those changes to the file.  The acl1$ to \
aclN$ arguments are strings of the form \"username+r\" or \"username-r\", where the former adds \
\"username\" to the users allowed to read the file, and the latter removes \"username\" from the \
list of users allowed to read the file.
You can use the special \"public+r\" ACL to share a file with everyone.
Note that this command only works for cloud-based drives as it is designed to share files \
among users of the EndBASIC service.",
                )
                .build(),
            console,
            storage,
        })
    }
}

impl ShareCommand {
    /// Parses a textual ACL specification and adds it to `add` or `remove.
    fn parse_acl(mut acl: String, add: &mut FileAcls, remove: &mut FileAcls) -> CommandResult {
        let change = if acl.len() < 3 { String::new() } else { acl.split_off(acl.len() - 2) };
        let username = acl; // For clarity after splitting off the ACL change request.
        match (username, change.as_str()) {
            (username, "+r") if !username.is_empty() => add.add_reader(username),
            (username, "+R") if !username.is_empty() => add.add_reader(username),
            (username, "-r") if !username.is_empty() => remove.add_reader(username),
            (username, "-R") if !username.is_empty() => remove.add_reader(username),
            (username, change) => {
                return Err(CallError::ArgumentError(format!(
                    "Invalid ACL '{}{}': must be of the form \"username+r\" or \"username-r\"",
                    username, change
                )))
            }
        }
        Ok(())
    }

    /// Fetches and prints the ACLs for `filename`.
    async fn show_acls(&self, filename: &str) -> CommandResult {
        let acls = self.storage.borrow().get_acls(filename).await?;

        let mut console = self.console.borrow_mut();
        console.print("")?;
        if acls.readers().is_empty() {
            console.print(&format!("    No ACLs on {}", filename))?;
        } else {
            console.print(&format!("    Reader ACLs on {}:", filename))?;
            for acl in acls.readers() {
                console.print(&format!("    {}", acl))?;
            }
        }
        console.print("")?;

        Ok(())
    }
}

#[async_trait(?Send)]
impl Command for ShareCommand {
    fn metadata(&self) -> &CallableMetadata {
        &self.metadata
    }

    async fn exec(&self, args: &[(Option<Expr>, ArgSep)], machine: &mut Machine) -> CommandResult {
        if args.is_empty() {
            return Err(CallError::ArgumentError(
                "SHARE requires one or more arguments".to_owned(),
            ));
        }

        let filename = match &args[0].0 {
            Some(e) => match e.eval(machine.get_mut_symbols()).await? {
                Value::Text(t) => t,
                _ => {
                    return Err(CallError::ArgumentError(
                        "SHARE requires a string as the filename".to_owned(),
                    ))
                }
            },
            None => {
                return Err(CallError::ArgumentError(
                    "SHARE requires a string as the filename".to_owned(),
                ))
            }
        };
        if args[0].1 == ArgSep::End {
            return self.show_acls(&filename).await;
        } else if args[0].1 != ArgSep::Long {
            return Err(CallError::ArgumentError(
                "SHARE requires arguments to be separated by commas".to_owned(),
            ));
        }

        let mut add = FileAcls::default();
        let mut remove = FileAcls::default();
        for arg in &args[1..] {
            match arg {
                (None, _) => {
                    return Err(CallError::ArgumentError(
                        "SHARE arguments cannot be empty".to_owned(),
                    ))
                }
                (_, ArgSep::Short) => {
                    return Err(CallError::ArgumentError(
                        "SHARE requires arguments to be separated by commas".to_owned(),
                    ))
                }
                (Some(acl), _) => match acl.eval(machine.get_mut_symbols()).await? {
                    Value::Text(t) => ShareCommand::parse_acl(t, &mut add, &mut remove)?,
                    _ => {
                        return Err(CallError::ArgumentError(
                            "SHARE requires strings as ACL changes".to_owned(),
                        ))
                    }
                },
            }
        }
        self.storage.borrow_mut().update_acls(&filename, &add, &remove).await?;
        Ok(())
    }
}

/// Adds all remote manipulation commands for `service` to the `machine`, using `console` to
/// display information and `storage` to manipulate the remote drives.
pub fn add_all(
    machine: &mut Machine,
    service: Rc<RefCell<dyn Service>>,
    console: Rc<RefCell<dyn Console>>,
    storage: Rc<RefCell<Storage>>,
) {
    machine.add_command(LoginCommand::new(service, console.clone(), storage.clone()));
    machine.add_command(ShareCommand::new(console, storage));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutils::*;
    use endbasic_std::testutils::*;

    #[test]
    fn test_login_ok_with_password() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse { access_token: AccessToken::new("random token"), motd: vec![] }),
        );
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
        t.run(format!(r#"LOGIN "{}", "{}""#, "the-username", "the-password")).check();
        assert!(t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_ok_ask_password() {
        let t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse { access_token: AccessToken::new("random token"), motd: vec![] }),
        );
        let storage = t.get_storage();
        assert!(!storage.borrow().mounted().contains_key("CLOUD"));

        t.get_console().borrow_mut().set_interactive(true);
        let mut exp_output = vec![CapturedOut::Write(b"Password: ".to_vec()), CapturedOut::SyncNow];
        for _ in 0.."the-password".len() {
            exp_output.push(CapturedOut::Write(vec![b'*']));
        }
        exp_output.push(CapturedOut::Print("".to_owned()));

        t.add_input_chars("the-password")
            .add_input_chars("\n")
            .run(format!(r#"LOGIN "{}""#, "the-username"))
            .expect_output(exp_output)
            .check();

        assert!(storage.borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_show_motd() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse {
                access_token: AccessToken::new("random token"),
                motd: vec!["first line".to_owned(), "second line".to_owned()],
            }),
        );
        t.run(format!(r#"LOGIN "{}", "{}""#, "the-username", "the-password"))
            .expect_prints([
                "",
                "----- BEGIN SERVER MOTD -----",
                "first line",
                "second line",
                "-----  END SERVER MOTD  -----",
                "",
            ])
            .check();
    }

    #[test]
    fn test_login_bad_credentials() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "bad-user",
            "the-password",
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "Unknown user")),
        );
        t.run(format!(r#"LOGIN "{}", "{}""#, "bad-user", "the-password"))
            .expect_err("Unknown user")
            .check();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "bad-password",
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "Invalid password")),
        );
        t.run(format!(r#"LOGIN "{}", "{}""#, "the-username", "bad-password"))
            .expect_err("Invalid password")
            .check();
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_twice_not_supported() {
        let mut t = ClientTester::default();
        t.get_service().borrow_mut().add_mock_login(
            "the-username",
            "the-password",
            Ok(LoginResponse { access_token: AccessToken::new("random token"), motd: vec![] }),
        );
        assert!(!t.get_storage().borrow().mounted().contains_key("CLOUD"));
        t.run(format!(r#"LOGIN "{}", "{}": LOGIN "a", "b""#, "the-username", "the-password"))
            .expect_err("Support for calling LOGIN twice in the same session is not implemented")
            .check();
        assert!(t.get_storage().borrow().mounted().contains_key("CLOUD"));
    }

    #[test]
    fn test_login_errors() {
        client_check_stmt_err("LOGIN requires one or two arguments", r#"LOGIN"#);
        client_check_stmt_err("LOGIN requires one or two arguments", r#"LOGIN "a", "b", "c""#);
        client_check_stmt_err("LOGIN requires a string as the username", r#"LOGIN 3"#);
        client_check_stmt_err("LOGIN requires a string as the username", r#"LOGIN 3, "a""#);
        client_check_stmt_err("LOGIN requires a string as the password", r#"LOGIN "a", 3"#);
    }

    #[test]
    fn test_share_parse_acl_ok() {
        let mut add = FileAcls::default();
        let mut remove = FileAcls::default();

        ShareCommand::parse_acl("user1+r".to_owned(), &mut add, &mut remove).unwrap();
        ShareCommand::parse_acl("user2+R".to_owned(), &mut add, &mut remove).unwrap();
        ShareCommand::parse_acl("X-r".to_owned(), &mut add, &mut remove).unwrap();
        ShareCommand::parse_acl("Y-R".to_owned(), &mut add, &mut remove).unwrap();
        assert_eq!(&["user1".to_owned(), "user2".to_owned()], add.readers());
        assert_eq!(&["X".to_owned(), "Y".to_owned()], remove.readers());
    }

    #[test]
    fn test_share_parse_acl_errors() {
        let mut add = FileAcls::default().with_readers(["before1".to_owned()]);
        let mut remove = FileAcls::default().with_readers(["before2".to_owned()]);

        for acl in &["", "r", "+r", "-r", "foo+", "bar-"] {
            let err = ShareCommand::parse_acl(acl.to_string(), &mut add, &mut remove).unwrap_err();
            let message = format!("{:?}", err);
            assert!(message.contains("Invalid ACL"));
            assert!(message.contains(acl));
        }

        assert_eq!(&["before1".to_owned()], add.readers());
        assert_eq!(&["before2".to_owned()], remove.readers());
    }

    #[tokio::test]
    async fn test_share_print_no_acls() {
        let mut t = ClientTester::default();
        t.get_storage().borrow_mut().put("MEMORY:/FOO", "").await.unwrap();
        t.run(r#"SHARE "MEMORY:/FOO""#)
            .expect_prints(["", "    No ACLs on MEMORY:/FOO", ""])
            .expect_file("MEMORY:/FOO", "")
            .check();
    }

    #[tokio::test]
    async fn test_share_print_some_acls() {
        let mut t = ClientTester::default();
        {
            let storage = t.get_storage();
            let mut storage = storage.borrow_mut();
            storage.put("MEMORY:/FOO", "").await.unwrap();
            storage
                .update_acls(
                    "MEMORY:/FOO",
                    &FileAcls::default().with_readers(["some".to_owned(), "person".to_owned()]),
                    &FileAcls::default(),
                )
                .await
                .unwrap();
        }
        t.run(r#"SHARE "MEMORY:/FOO""#)
            .expect_prints(["", "    Reader ACLs on MEMORY:/FOO:", "    person", "    some", ""])
            .expect_file("MEMORY:/FOO", "")
            .check();
    }

    #[test]
    fn test_share_errors() {
        client_check_stmt_err("SHARE requires one or more arguments", r#"SHARE"#);
        client_check_stmt_err("SHARE requires a string as the filename", r#"SHARE 1"#);
        client_check_stmt_err("SHARE requires a string as the filename", r#"SHARE , "a""#);
        client_check_stmt_err(
            "SHARE requires arguments to be separated by commas",
            r#"SHARE "a"; "b""#,
        );
        client_check_stmt_err(
            "SHARE requires arguments to be separated by commas",
            r#"SHARE "a", "b"; "c""#,
        );
        client_check_stmt_err("SHARE arguments cannot be empty", r#"SHARE "a", , "b""#);
        client_check_stmt_err("SHARE requires strings as ACL changes", r#"SHARE "a", 3, "b""#);
        client_check_stmt_err(
            r#"Invalid ACL 'foobar': must be of the form "username+r" or "username-r""#,
            r#"SHARE "a", "foobar""#,
        );
    }
}
