use super::*;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Step {
    #[serde(rename = "git.clone")]
    Clone,
    #[serde(rename = "git.merge-if-configured")]
    Merge,
    #[serde(rename = "git.push")]
    Push,
    #[serde(rename = "zip.archive")]
    Archive,
    #[serde(rename = "fs.cleanup-clone")]
    Cleanup,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum FinalStep {
    #[serde(rename = "svn.commit-archives")]
    Commit,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Workflow {
    pub per_repository: Vec<Step>,
    pub after_repositories: Vec<FinalStep>,
}
impl Workflow {
    pub fn validate(&self) -> Result<()> {
        if self.per_repository.first() != Some(&Step::Clone)
            || self.after_repositories != [FinalStep::Commit]
        {
            return Err("同步工作流必须先克隆，并在所有仓库处理后提交 SVN 归档".into());
        }
        let mut last = 0;
        for (i, step) in self.per_repository.iter().enumerate() {
            let order = match step {
                Step::Clone => 0,
                Step::Merge => 1,
                Step::Push => 2,
                Step::Archive => 3,
                Step::Cleanup => 4,
            };
            if i > 0 && order <= last {
                return Err("工作流操作重复或顺序无效".into());
            }
            last = order;
        }
        if !self.per_repository.contains(&Step::Archive) {
            return Err("同步工作流缺少 ZIP 归档步骤".into());
        }
        Ok(())
    }
}
