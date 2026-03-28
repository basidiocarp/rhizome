use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use lsp_types::{
    DocumentChangeOperation, DocumentChanges, OneOf, Position, ResourceOp, TextDocumentEdit,
    TextEdit, Uri, WorkspaceEdit,
};
use tempfile::NamedTempFile;

use crate::convert::uri_to_file_path;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ApplyResult {
    pub files_modified: usize,
    pub edits_applied: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PreviewResult {
    pub files_modified: usize,
    pub edits_applied: usize,
    pub affected_paths: Vec<String>,
}

pub fn apply_workspace_edit(edit: &WorkspaceEdit) -> Result<ApplyResult> {
    let mut result = ApplyResult::default();

    if let Some(changes) = &edit.changes {
        for (uri, edits) in changes {
            apply_text_edits_to_uri(uri, edits, &mut result)?;
        }
    }

    if let Some(document_changes) = &edit.document_changes {
        match document_changes {
            DocumentChanges::Edits(edits) => {
                for edit in edits {
                    apply_text_document_edit(edit, &mut result)?;
                }
            }
            DocumentChanges::Operations(ops) => {
                for op in ops {
                    match op {
                        DocumentChangeOperation::Edit(edit) => {
                            apply_text_document_edit(edit, &mut result)?;
                        }
                        DocumentChangeOperation::Op(resource_op) => {
                            apply_resource_op(resource_op, &mut result)?;
                        }
                    }
                }
            }
        }
    }

    Ok(result)
}

pub fn summarize_workspace_edit(edit: &WorkspaceEdit) -> Result<PreviewResult> {
    let mut result = ApplyResult::default();
    let mut affected_paths = BTreeSet::new();

    if let Some(changes) = &edit.changes {
        for (uri, edits) in changes {
            summarize_text_edits_to_uri(uri, edits, &mut result, &mut affected_paths)?;
        }
    }

    if let Some(document_changes) = &edit.document_changes {
        match document_changes {
            DocumentChanges::Edits(edits) => {
                for edit in edits {
                    summarize_text_document_edit(edit, &mut result, &mut affected_paths)?;
                }
            }
            DocumentChanges::Operations(ops) => {
                for op in ops {
                    match op {
                        DocumentChangeOperation::Edit(edit) => {
                            summarize_text_document_edit(edit, &mut result, &mut affected_paths)?;
                        }
                        DocumentChangeOperation::Op(resource_op) => {
                            summarize_resource_op(resource_op, &mut result, &mut affected_paths)?;
                        }
                    }
                }
            }
        }
    }

    Ok(PreviewResult {
        files_modified: result.files_modified,
        edits_applied: result.edits_applied,
        affected_paths: affected_paths.into_iter().collect(),
    })
}

fn apply_text_document_edit(edit: &TextDocumentEdit, result: &mut ApplyResult) -> Result<()> {
    let uri = &edit.text_document.uri;
    let edits: Vec<TextEdit> = edit
        .edits
        .iter()
        .map(|edit| match edit {
            OneOf::Left(text_edit) => text_edit.clone(),
            OneOf::Right(annotated) => annotated.text_edit.clone(),
        })
        .collect();
    apply_text_edits_to_uri(uri, &edits, result)
}

fn summarize_text_document_edit(
    edit: &TextDocumentEdit,
    result: &mut ApplyResult,
    affected_paths: &mut BTreeSet<String>,
) -> Result<()> {
    let uri = &edit.text_document.uri;
    let edits: Vec<TextEdit> = edit
        .edits
        .iter()
        .map(|edit| match edit {
            OneOf::Left(text_edit) => text_edit.clone(),
            OneOf::Right(annotated) => annotated.text_edit.clone(),
        })
        .collect();
    summarize_text_edits_to_uri(uri, &edits, result, affected_paths)
}

fn apply_text_edits_to_uri(uri: &Uri, edits: &[TextEdit], result: &mut ApplyResult) -> Result<()> {
    let path = PathBuf::from(uri_to_file_path(uri));
    apply_text_edits(&path, edits)?;
    result.files_modified += 1;
    result.edits_applied += edits.len();
    Ok(())
}

fn summarize_text_edits_to_uri(
    uri: &Uri,
    edits: &[TextEdit],
    result: &mut ApplyResult,
    affected_paths: &mut BTreeSet<String>,
) -> Result<()> {
    let path = uri_to_file_path(uri);
    affected_paths.insert(path);
    result.files_modified += 1;
    result.edits_applied += edits.len();
    Ok(())
}

fn apply_resource_op(op: &ResourceOp, result: &mut ApplyResult) -> Result<()> {
    match op {
        ResourceOp::Create(create) => {
            let path = PathBuf::from(uri_to_file_path(&create.uri));
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent directory {}", parent.display()))?;
            }

            let exists = path.exists();
            let overwrite = create
                .options
                .as_ref()
                .and_then(|o| o.overwrite)
                .unwrap_or(false);
            let ignore_if_exists = create
                .options
                .as_ref()
                .and_then(|o| o.ignore_if_exists)
                .unwrap_or(false);

            if exists && !overwrite {
                if ignore_if_exists {
                    return Ok(());
                }
                anyhow::bail!("cannot create {}, file already exists", path.display());
            }

            atomic_write(&path, "")?;
            result.files_modified += 1;
        }
        ResourceOp::Rename(rename) => {
            let old_path = PathBuf::from(uri_to_file_path(&rename.old_uri));
            let new_path = PathBuf::from(uri_to_file_path(&rename.new_uri));
            let overwrite = rename
                .options
                .as_ref()
                .and_then(|o| o.overwrite)
                .unwrap_or(false);
            let ignore_if_exists = rename
                .options
                .as_ref()
                .and_then(|o| o.ignore_if_exists)
                .unwrap_or(false);

            if !old_path.exists() {
                anyhow::bail!(
                    "cannot rename {}, source file does not exist",
                    old_path.display()
                );
            }
            if new_path.exists() && !overwrite {
                if ignore_if_exists {
                    return Ok(());
                }
                anyhow::bail!(
                    "cannot rename to {}, target file already exists",
                    new_path.display()
                );
            }
            if let Some(parent) = new_path.parent() {
                fs::create_dir_all(parent)
                    .with_context(|| format!("creating parent directory {}", parent.display()))?;
            }
            if new_path.exists() && overwrite {
                if new_path.is_dir() {
                    fs::remove_dir_all(&new_path).with_context(|| {
                        format!("removing existing directory {}", new_path.display())
                    })?;
                } else {
                    fs::remove_file(&new_path).with_context(|| {
                        format!("removing existing file {}", new_path.display())
                    })?;
                }
            }
            fs::rename(&old_path, &new_path).with_context(|| {
                format!("renaming {} to {}", old_path.display(), new_path.display())
            })?;
            result.files_modified += 1;
        }
        ResourceOp::Delete(delete) => {
            let path = PathBuf::from(uri_to_file_path(&delete.uri));
            if !path.exists() {
                if delete
                    .options
                    .as_ref()
                    .and_then(|o| o.ignore_if_not_exists)
                    .unwrap_or(false)
                {
                    return Ok(());
                }
                anyhow::bail!("cannot delete {}, path does not exist", path.display());
            }
            if path.is_dir() {
                if delete
                    .options
                    .as_ref()
                    .and_then(|o| o.recursive)
                    .unwrap_or(false)
                {
                    fs::remove_dir_all(&path)
                        .with_context(|| format!("removing directory {}", path.display()))?;
                } else {
                    fs::remove_dir(&path)
                        .with_context(|| format!("removing directory {}", path.display()))?;
                }
            } else {
                fs::remove_file(&path)
                    .with_context(|| format!("removing file {}", path.display()))?;
            }
            result.files_modified += 1;
        }
    }
    Ok(())
}

fn summarize_resource_op(
    op: &ResourceOp,
    result: &mut ApplyResult,
    affected_paths: &mut BTreeSet<String>,
) -> Result<()> {
    match op {
        ResourceOp::Create(create) => {
            affected_paths.insert(uri_to_file_path(&create.uri));
            result.files_modified += 1;
        }
        ResourceOp::Rename(rename) => {
            affected_paths.insert(uri_to_file_path(&rename.old_uri));
            affected_paths.insert(uri_to_file_path(&rename.new_uri));
            result.files_modified += 1;
        }
        ResourceOp::Delete(delete) => {
            affected_paths.insert(uri_to_file_path(&delete.uri));
            result.files_modified += 1;
        }
    }
    Ok(())
}

fn apply_text_edits(path: &Path, edits: &[TextEdit]) -> Result<()> {
    let mut content = if path.exists() {
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
    } else {
        String::new()
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating parent directory {}", parent.display()))?;
    }

    let mut spans = Vec::with_capacity(edits.len());
    for edit in edits {
        let start = position_to_offset(&content, edit.range.start)
            .with_context(|| format!("invalid start range for {}", path.display()))?;
        let end = position_to_offset(&content, edit.range.end)
            .with_context(|| format!("invalid end range for {}", path.display()))?;
        if start > end {
            anyhow::bail!("invalid edit range for {}", path.display());
        }
        spans.push((start, end, edit.new_text.as_str()));
    }

    spans.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| b.1.cmp(&a.1)));

    let mut previous_start = None;
    for (start, end, replacement) in spans {
        if let Some(prev_start) = previous_start {
            if end > prev_start {
                anyhow::bail!("overlapping workspace edits for {}", path.display());
            }
        }
        content.replace_range(start..end, replacement);
        previous_start = Some(start);
    }

    atomic_write(path, &content)
}

fn atomic_write(path: &Path, content: &str) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("cannot determine parent directory for {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("creating parent directory {}", parent.display()))?;

    let mut temp = NamedTempFile::new_in(parent)
        .with_context(|| format!("creating temporary file in {}", parent.display()))?;
    use std::io::Write as _;
    temp.write_all(content.as_bytes())
        .with_context(|| format!("writing temporary file for {}", path.display()))?;
    temp.flush()
        .with_context(|| format!("flushing temporary file for {}", path.display()))?;
    temp.persist(path)
        .map_err(|err| anyhow!("persisting {} failed: {}", path.display(), err.error))?;
    Ok(())
}

fn position_to_offset(text: &str, position: Position) -> Result<usize> {
    let mut line_start = 0usize;
    let mut current_line = 0u32;

    for segment in text.split_inclusive('\n') {
        let line_body = segment.strip_suffix('\n').unwrap_or(segment);
        if current_line == position.line {
            return utf16_column_to_offset(line_body, position.character)
                .map(|column_offset| line_start + column_offset);
        }
        line_start += segment.len();
        current_line += 1;
    }

    if current_line == position.line {
        return utf16_column_to_offset("", position.character).map(|offset| line_start + offset);
    }

    Err(anyhow!(
        "line {} is out of bounds for document with {} line(s)",
        position.line,
        current_line + 1
    ))
}

fn utf16_column_to_offset(line: &str, column: u32) -> Result<usize> {
    let target = column as usize;
    let mut utf16_units = 0usize;

    for (byte_offset, ch) in line.char_indices() {
        if utf16_units == target {
            return Ok(byte_offset);
        }

        let char_units = ch.len_utf16();
        if utf16_units + char_units > target {
            anyhow::bail!("column {} splits a multi-unit character", column);
        }
        utf16_units += char_units;
    }

    if utf16_units == target {
        Ok(line.len())
    } else {
        Err(anyhow!(
            "column {} is out of bounds for line with {} UTF-16 units",
            column,
            utf16_units
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use lsp_types::{DocumentChanges, OneOf, OptionalVersionedTextDocumentIdentifier, Range};
    use tempfile::TempDir;

    use super::*;
    use crate::convert::path_to_lsp_uri;

    fn range(start_line: u32, start_char: u32, end_line: u32, end_char: u32) -> lsp_types::Range {
        Range {
            start: Position {
                line: start_line,
                character: start_char,
            },
            end: Position {
                line: end_line,
                character: end_char,
            },
        }
    }

    #[test]
    fn apply_workspace_edit_replaces_text_in_single_file() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("main.rs");
        fs::write(&file, "fn main() {\n    println!(\"old\");\n}\n").unwrap();

        let uri = path_to_lsp_uri(&file).unwrap();
        let mut changes = HashMap::new();
        changes.insert(
            uri,
            vec![TextEdit::new(range(1, 14, 1, 17), "new".to_string())],
        );

        let result = apply_workspace_edit(&WorkspaceEdit::new(changes)).unwrap();
        assert_eq!(result.files_modified, 1);
        assert_eq!(result.edits_applied, 1);
        assert!(fs::read_to_string(&file).unwrap().contains("\"new\""));
    }

    #[test]
    fn apply_workspace_edit_handles_multiple_edits_in_reverse_order() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("lib.rs");
        fs::write(&file, "alpha\nbeta\ngamma\n").unwrap();

        let uri = path_to_lsp_uri(&file).unwrap();
        let edit = WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Edits(vec![TextDocumentEdit {
                text_document: OptionalVersionedTextDocumentIdentifier { uri, version: None },
                edits: vec![
                    OneOf::Left(TextEdit::new(range(2, 0, 2, 5), "delta".to_string())),
                    OneOf::Left(TextEdit::new(range(0, 0, 0, 5), "omega".to_string())),
                ],
            }])),
            change_annotations: None,
        };

        let result = apply_workspace_edit(&edit).unwrap();
        assert_eq!(result.files_modified, 1);
        assert_eq!(result.edits_applied, 2);
        assert_eq!(fs::read_to_string(&file).unwrap(), "omega\nbeta\ndelta\n");
    }

    #[test]
    fn apply_workspace_edit_creates_missing_parent_directories() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("nested/src/lib.rs");
        let uri = path_to_lsp_uri(&file).unwrap();

        let mut changes = HashMap::new();
        changes.insert(
            uri,
            vec![TextEdit::new(
                range(0, 0, 0, 0),
                "pub fn created() {}\n".to_string(),
            )],
        );

        let result = apply_workspace_edit(&WorkspaceEdit::new(changes)).unwrap();
        assert_eq!(result.files_modified, 1);
        assert_eq!(result.edits_applied, 1);
        assert_eq!(fs::read_to_string(&file).unwrap(), "pub fn created() {}\n");
    }

    #[test]
    fn summarize_workspace_edit_reports_targets_without_writing_files() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("main.rs");
        fs::write(&file, "fn main() {}\n").unwrap();

        let uri = path_to_lsp_uri(&file).unwrap();
        let mut changes = HashMap::new();
        changes.insert(
            uri,
            vec![TextEdit::new(range(0, 3, 0, 7), "start".to_string())],
        );

        let preview = summarize_workspace_edit(&WorkspaceEdit::new(changes)).unwrap();
        assert_eq!(preview.files_modified, 1);
        assert_eq!(preview.edits_applied, 1);
        assert_eq!(preview.affected_paths, vec![file.display().to_string()]);
        assert_eq!(fs::read_to_string(&file).unwrap(), "fn main() {}\n");
    }

    #[test]
    fn summarize_workspace_edit_tracks_rename_targets() {
        let dir = TempDir::new().unwrap();
        let old_path = dir.path().join("old.rs");
        let new_path = dir.path().join("new.rs");
        let edit = WorkspaceEdit {
            changes: None,
            document_changes: Some(DocumentChanges::Operations(vec![
                DocumentChangeOperation::Op(ResourceOp::Rename(lsp_types::RenameFile {
                    old_uri: path_to_lsp_uri(&old_path).unwrap(),
                    new_uri: path_to_lsp_uri(&new_path).unwrap(),
                    options: None,
                    annotation_id: None,
                })),
            ])),
            change_annotations: None,
        };

        let preview = summarize_workspace_edit(&edit).unwrap();
        assert_eq!(preview.files_modified, 1);
        assert_eq!(preview.edits_applied, 0);
        assert_eq!(
            preview.affected_paths,
            vec![
                new_path.display().to_string(),
                old_path.display().to_string()
            ]
        );
    }
}
