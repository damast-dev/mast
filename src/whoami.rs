use clap::{self, Args};
use eyre::{Context, OptionExt};

use crate::{KeyInfo, ftl_println, repo::RepoInfo};

#[derive(Args, Clone, Debug)]
pub struct WhoAmICommand {
    #[clap(long, short)]
    remote: Option<String>,
}

impl WhoAmICommand {
    pub async fn run(self, keys: &mut KeyInfo, host_name: Option<&str>) -> eyre::Result<()> {
        let url = RepoInfo::get_current(host_name, None, self.remote.as_deref(), keys)
            .wrap_err("could not find host, try specifying with --host")?
            .host_url()
            .clone();
        let host = crate::host_name(&url);
        let api = keys.get_api(&url).await?;
        let current_user = api.user_get_current().await?;
        let name = current_user
            .login
            .ok_or_eyre("user does not have login name")?;
        ftl_println!("msg-whoami", name, host);
        Ok(())
    }
}
