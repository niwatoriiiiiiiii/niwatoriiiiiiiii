use anyhow::{Result, anyhow};
use pulldown_cmark::{Options, Parser, html};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context, Tera};
use walkdir::WalkDir;

/// Represents a blog post and its associated metadata
#[derive(Serialize, Deserialize, Clone)]
struct Post {
    title: String,
    date: String,
    #[serde(skip)]
    body: String,
    // Relative path to the output index.html (e.g. "my-post/index.html")
    out_rel_path: String, 
}

fn main() -> Result<()> {
    // Setup
    let tera = Tera::new("templates/**/*.html")?;
    let mut posts = Vec::new();
    let articles_dir = Path::new("articles");

    // Clean up and recreate docs/articles
    if Path::new("docs/articles").exists() {
        fs::remove_dir_all("docs/articles")?;
    }
    fs::create_dir_all("docs/articles")?;

    // Collect articles and assets
    for entry in WalkDir::new(articles_dir).into_iter().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.is_dir() { continue; }

        let rel_path = path.strip_prefix(articles_dir)?;
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if file_name.ends_with(".md") {
            // Process markdown files
            let file_content = fs::read_to_string(path)?;
            let (metadata, body) = parse_frontmatter(&file_content)?;
            
            // Generate output path: articles/folder/index.html
            let out_rel_path = if rel_path.components().count() > 1 {
                // If in a subfolder, use folder name as slug
                rel_path.parent().unwrap().join("index.html")
            } else {
                // If directly in articles/, use filename as slug
                rel_path.with_extension("").join("index.html")
            };
            
            posts.push(Post {
                title: metadata.title,
                date: metadata.date,
                body: body.to_string(),
                out_rel_path: out_rel_path.to_string_lossy().to_string(),
            });
        } else if !file_name.starts_with('.') {
            // Copy assets (images, etc.)
            let mut out_path = PathBuf::from("docs/articles");
            out_path.push(rel_path);
            
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::copy(path, out_path)?;
        }
    }

    // Sort by date desc, then by title desc
    posts.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.title.cmp(&a.title)));

    // Generate article pages
    for (i, post) in posts.iter().enumerate() {
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        options.insert(Options::ENABLE_FOOTNOTES);
        options.insert(Options::ENABLE_STRIKETHROUGH);
        options.insert(Options::ENABLE_TASKLISTS);
        options.insert(Options::ENABLE_HEADING_ATTRIBUTES);
        
        let parser = Parser::new_ext(&post.body, options);
        let mut html_output = String::new();
        html::push_html(&mut html_output, parser);

        // Remove disabled attribute from checkboxes for custom styling
        let html_output = html_output.replace("disabled=\"\"", "").replace("disabled", "");

        // Navigation (next/prev)
        let prev_post = if i + 1 < posts.len() { Some(&posts[i + 1]) } else { None };
        let next_post = if i > 0 { Some(&posts[i - 1]) } else { None };

        // Calculate base_url depth (articles/slug/index.html is depth 2)
        let depth = Path::new(&post.out_rel_path).components().count();
        let base_url = "../".repeat(depth);

        let mut context = Context::new();
        context.insert("title", &post.title);
        context.insert("page_title", &post.title);
        context.insert("body", &html_output);
        context.insert("date", &post.date);
        context.insert("base_url", &base_url);
        context.insert("prev_post", &prev_post);
        context.insert("next_post", &next_post);

        let rendered = tera.render("article.html", &context)?;
        
        let mut out_full_path = PathBuf::from("docs/articles");
        out_full_path.push(&post.out_rel_path);
        
        if let Some(parent) = out_full_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(out_full_path, rendered)?;
    }

    // Generate index page
    let mut index_context = Context::new();
    index_context.insert("posts", &posts);
    index_context.insert("base_url", "./");

    let index_rendered = tera.render("index.html", &index_context)?;
    fs::write("docs/index.html", index_rendered)?;

    // Copy static assets
    if Path::new("static/assets").exists() {
        let mut options = fs_extra::dir::CopyOptions::new();
        options.overwrite = true;
        fs_extra::dir::copy("static/assets", "docs", &options)?;
    }

    println!("Success! Processed {} articles.", posts.len());
    Ok(())
}

/// Simple parser for YAML frontmatter (--- ... ---)
fn parse_frontmatter(content: &str) -> Result<(PostMetadata, &str)> {
    if !content.starts_with("---") {
        return Err(anyhow!("Frontmatter (---) not found."));
    }

    let rest = &content[3..];
    let end = rest
        .find("---")
        .ok_or_else(|| anyhow!("Closing frontmatter (---) not found."))?;

    let yaml = &rest[..end];
    let body = &rest[end + 3..];

    let metadata: PostMetadata = serde_yaml::from_str(yaml)?;

    Ok((metadata, body.trim()))
}

#[derive(Serialize, Deserialize)]
struct PostMetadata {
    title: String,
    date: String,
}
