use crate::resp_types::{RespLoginInfo, RespMinimalAuthorInfo};

pub fn abbreviate_link(href: &str) -> &str {
    // Attempt to find the hostname from the URL
    match href.find("://") {
        Some(idx1) => match href[(idx1 + 3)..].find('/') {
            Some(idx2) => Some(&href[(idx1 + 3)..(idx1 + 3 + idx2)]),
            None => None,
        },
        None => None,
    }
    .unwrap_or(href)
}

pub fn author_is_me(
    author: &Option<RespMinimalAuthorInfo<'_>>,
    login: &Option<RespLoginInfo>,
) -> bool {
    if let Some(author) = author {
        if let Some(login) = login {
            if author.id == login.user.id {
                return true;
            }
        }
    }
    false
}
