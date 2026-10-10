//! Declaration-boundary facade splitting preserves public inherited call surfaces.
use anyhow::Result;
use poolster_core::GeneratedFile;

/// Each method is emitted as one complete declaration by the operation renderer.
/// Parts own declarations, rather than splitting arbitrary bytes inside a method.
pub(super) fn client(
    package: &str,
    calls: &[String],
    groups: &[String],
) -> Result<Vec<GeneratedFile>> {
    let root = format!("src/main/java/{}", package.replace('.', "/"));
    let mut files = Vec::new();
    let calls_text = calls.join("\n");
    let groups_text = groups.join("\n");
    if calls_text.len() + groups_text.len() < 32 * 1024 && calls.len() + groups.len() <= 50 {
        files.push(GeneratedFile::new(format!("{root}/Client.java"),format!("package {package};\n\nimport {package}.models.*;\n\nimport {package}.operations.*;\n\npublic final class Client extends GraphqlRuntime {{\n  public Client(Transport transport){{\n    super(transport);\n\n  }}\n  public Client(String endpoint){{\n    this(new HttpTransport(endpoint));\n\n  }}\n  \n{calls_text}\n{groups_text}\n}}\n\n"))?);
        return Ok(files);
    }
    let all = calls
        .iter()
        .cloned()
        .chain(
            groups
                .iter()
                .map(|group| group.replace("(this)", "((Client)this)")),
        )
        .collect::<Vec<_>>();
    let mut parent = "GraphqlRuntime".to_owned();
    for (index, chunk) in chunks(&all).into_iter().enumerate() {
        let name = format!("GraphqlClientPart{index:03}");
        files.push(GeneratedFile::new(
            format!("{root}/{name}.java"),
            format!("package {package};\n\nimport {package}.models.*;\n\nimport {package}.operations.*;\n\npublic class {name} extends {parent} {{\n  protected {name}(Transport transport) {{\n    super(transport);\n\n  }}\n  \n{chunk}\n}}\n\n"),
        )?);
        parent = name;
    }
    files.push(GeneratedFile::new(format!("{root}/Client.java"),format!("package {package};\n\npublic final class Client extends {parent} {{\n  public Client(Transport transport) {{\n    super(transport);\n\n  }}\n  \n  public Client(String endpoint) {{\n    this(new HttpTransport(endpoint));\n\n  }}\n  \n}}\n\n"))?);
    Ok(files)
}
pub(super) fn group(
    package: &str,
    name: &str,
    source: &str,
    declarations: &[String],
) -> Result<Vec<GeneratedFile>> {
    let root = format!("src/main/java/{}/groups", package.replace('.', "/"));
    if source.len() < 32 * 1024 && declarations.len() <= 50 {
        return Ok(vec![GeneratedFile::new(
            format!("{root}/{name}.java"),
            source,
        )?]);
    }
    let mut parent = None::<String>;
    let mut files = Vec::new();
    for (index, chunk) in chunks(declarations).into_iter().enumerate() {
        let class = format!("{name}Part{index:03}");
        let (extends, field, constructor) = match &parent {
            Some(parent) => (
                format!(" extends {parent}"),
                String::new(),
                "super(client);",
            ),
            None => (
                String::new(),
                "protected final Client client;".into(),
                "this.client=client;",
            ),
        };
        files.push(GeneratedFile::new(format!("{root}/{class}.java"),format!("package {package}.groups;\n\nimport {package}.Client;\n\nimport {package}.models.*;\n\nimport static {package}.GraphqlRuntime.*;\n\npublic class {class}{extends} {{\n  {field}\nprotected {class}(Client client) {{\n    {constructor}\n  }}\n{chunk}\n}}\n\n"))?);
        parent = Some(class);
    }
    let declaration = if let Some(parent) = parent {
        format!(
            "package {package}.groups;\n\nimport {package}.Client;\n\npublic final class {name} extends {parent} {{\n  public {name}(Client client) {{\n    super(client);\n\n  }}\n  \n}}\n\n"
        )
    } else {
        source.to_owned()
    };
    files.push(GeneratedFile::new(
        format!("{root}/{name}.java"),
        declaration,
    )?);
    Ok(files)
}
fn chunks(declarations: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    let mut chunk = String::new();
    let mut count = 0;
    for line in declarations {
        if count >= 50 || (!chunk.is_empty() && chunk.len() + line.len() > 32 * 1024) {
            result.push(std::mem::take(&mut chunk));
            count = 0;
        }
        chunk.push_str(line);
        chunk.push('\n');
        count += 1;
    }
    if !chunk.is_empty() {
        result.push(chunk)
    }
    result
}
