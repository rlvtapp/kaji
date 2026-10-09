use super::*;
pub(super) fn facade(
    files: &mut BTreeMap<String, String>,
    dir: &str,
    exports: &[(String, Vec<String>)],
) -> String {
    let mut nodes = vec![];
    for (index, items) in exports.chunks(32).enumerate() {
        let path = format!("_exports_leaf{index}");
        let mut code = String::new();
        let mut names = vec![];
        for (module, exports) in items {
            writeln!(code, "from .{module} import {}", exports.join(", ")).unwrap();
            names.extend(exports.iter().cloned());
        }
        writeln!(code, "__all__ = {:?}", names).unwrap();
        files.insert(format!("{dir}/{path}.py"), code);
        nodes.push(path);
    }
    let mut level = 0;
    while nodes.len() > 1 {
        let mut next = vec![];
        for (index, children) in nodes.chunks(2).enumerate() {
            if children.len() == 1 {
                next.push(children[0].clone());
                continue;
            }
            let path = format!("_exports_branch{level}_{index}");
            let code = format!(
                "from .{} import *\nfrom .{} import *\nfrom .{} import __all__ as _left\nfrom .{} import __all__ as _right\n__all__ = _left + _right\n",
                children[0], children[1], children[0], children[1]
            );
            files.insert(format!("{dir}/{path}.py"), code);
            next.push(path);
        }
        nodes = next;
        level += 1;
    }
    nodes
        .first()
        .map(|root| format!("from .{root} import *\nfrom .{root} import __all__\n"))
        .unwrap_or("__all__ = []\n".into())
}
pub(super) fn mixins(
    files: &mut BTreeMap<String, String>,
    owner: &str,
    bases: Vec<(String, String)>,
) -> Result<Option<(String, String)>> {
    let mut nodes = bases;
    let mut depth = 0;
    while nodes.len() > 1 {
        let mut next = vec![];
        for (i, children) in nodes.chunks(2).enumerate() {
            if children.len() == 1 {
                next.push(children[0].clone());
                continue;
            }
            let module = format!("{owner}_{depth}_{i}");
            let class = format!("_Branch{depth}_{i}");
            let mut code = String::new();
            for (j, (path, name)) in children.iter().enumerate() {
                writeln!(code, "from {path} import {name} as _Base{j}")?;
            }
            writeln!(code, "class {class}(_Base0, _Base1):\n    pass\n")?;
            files.insert(format!("_mixins/{module}.py"), code);
            next.push((format!(".{module}"), class));
        }
        nodes = next;
        depth += 1;
    }
    Ok(nodes.pop())
}
