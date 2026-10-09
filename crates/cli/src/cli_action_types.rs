use super::*;

pub(super) enum Action {
    Plan(Vec<OsString>),
    Eject(Vec<OsString>),
    Migrate(Vec<OsString>),
    Contract(Vec<OsString>),
    Sdk(sdk_automation::Options),
    Help,
    Version,
    Languages,
    Show(Show),
    Update(Update),
    Auth(Auth),
    Discover(Discover),
    Download(Download),
    Init(Init),
    Mcp(Mcp),
    McpGenerator,
    MockServe(MockServe),
    Check(Check),
    Generate(Box<Generate>),
}

#[derive(Debug)]
pub(super) struct Show {
    pub(super) source: PathBuf,
    pub(super) compiler: Option<PathBuf>,
    pub(super) paths: PathSelection,
    pub(super) format: ShowFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShowFormat {
    Human,
    Json,
}

#[derive(Debug)]
pub(super) struct Update {
    pub(super) output: PathBuf,
    pub(super) force: bool,
}

#[derive(Debug)]
pub(super) enum Auth {
    Login { profile: String, token_env: String },
    Logout { profile: String },
    Status,
}

#[derive(Debug)]
pub(super) struct Discover {
    pub(super) query: String,
    pub(super) limit: usize,
    pub(super) format: DiscoverFormat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DiscoverFormat {
    Human,
    Json,
}

#[derive(Debug)]
pub(super) struct Download {
    pub(super) id: String,
    pub(super) version: Option<String>,
    pub(super) output: PathBuf,
}
