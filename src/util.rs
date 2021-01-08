use crate::resp_types::{RespLoginInfo, RespMinimalAuthorInfo};

pub fn abbreviate_link(href: &str) -> &str {
    // Attempt to find the hostname from the URL
    href.find("://")
        .and_then(|idx1| {
            href[(idx1 + 3)..]
                .find('/')
                .map(|idx2| &href[(idx1 + 3)..(idx1 + 3 + idx2)])
        })
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
