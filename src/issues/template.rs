use forgejo_api::Forgejo;

pub mod yaml;

use crate::repo::RepoName;

#[derive(serde::Deserialize, Default, Debug)]
pub struct TemplateMetadata {
    pub labels: Option<Vec<String>>,
    pub r#ref: Option<String>,
}

pub struct MarkdownTemplate {
    pub metadata: TemplateMetadata,
    pub body: String,
}

impl MarkdownTemplate {
    pub fn new(md: &str) -> eyre::Result<Self> {
        let md_without_start = md
            .strip_prefix("---\n")
            .or_else(|| md.strip_prefix("---\r\n"));
        let stripped = md_without_start
            .and_then(|md| md.split_once("\n---\n"))
            .or_else(|| md_without_start.and_then(|md| md.split_once("\r\n---\r\n")));

        if let Some((front_matter, body)) = stripped {
            let metadata = serde_saphyr::from_str::<TemplateMetadata>(front_matter)?;

            Ok(Self {
                metadata,
                body: body.to_owned(),
            })
        } else {
            Ok(Self {
                metadata: TemplateMetadata::default(),
                body: md.to_owned(),
            })
        }
    }
}

pub async fn get_template_file(
    repo: &RepoName,
    api: &Forgejo,
    name: &str,
) -> eyre::Result<(Vec<u8>, bool)> {
    const DIRS: [&str; 8] = [
        ".forgejo/issue_template",
        ".forgejo/ISSUE_TEMPLATE",
        ".gitea/issue_template",
        ".gitea/ISSUE_TEMPLATE",
        ".github/issue_template",
        ".github/ISSUE_TEMPLATE",
        "docs/issue_template",
        "docs/ISSUE_TEMPLATE",
    ];
    const EXTS: [&str; 3] = ["md", "yml", "yaml"];
    let query = forgejo_api::structs::RepoGetRawFileQuery { r#ref: None };
    for dir in DIRS {
        for ext in EXTS {
            let path = format!("{dir}/{name}.{ext}");
            let file = api
                .repo_get_raw_file(repo.owner(), repo.name(), &path, query.clone())
                .await;
            match file {
                Ok(file) => {
                    let is_yaml = matches!(ext, "yml" | "yaml");
                    return Ok((file.to_vec(), is_yaml));
                }
                Err(forgejo_api::ForgejoError::ApiError(forgejo_api::ApiError {
                    kind: forgejo_api::ApiErrorKind::NotFound { .. },
                    ..
                })) => (),
                Err(e) => return Err(e.into()),
            }
        }
    }
    eyre::bail!("Could not find template '{name}'");
}

pub async fn generate_from_template(
    body: Option<String>,
    template_file: Vec<u8>,
    is_yaml: bool,
) -> eyre::Result<(String, TemplateMetadata)> {
    let template_file = std::str::from_utf8(&template_file)?;
    if is_yaml {
        let tmpl =
            serde_saphyr::from_str::<crate::issues::template::yaml::YamlTemplate>(template_file)?;

        let form = match body {
            Some(body) => body,
            None => {
                let mut form = tmpl.generate_form()?;
                crate::editor(&mut form, Some("md")).await?;
                form
            }
        };
        let body = tmpl.generate_content(tmpl.parse_form(&form)?)?;

        Ok((body, tmpl.metadata))
    } else {
        let mut tmpl = crate::issues::template::MarkdownTemplate::new(template_file)?;

        let body = match body {
            Some(body) => body,
            None => {
                crate::editor(&mut tmpl.body, Some("md")).await?;
                tmpl.body
            }
        };

        Ok((body, tmpl.metadata))
    }
}
