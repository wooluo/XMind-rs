//! xmind-cli — headless convert / render / info for xmind-rs.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn usage() {
    println!(
        "xmind-cli {} — 开源 XMind 命令行工具

用法:
  xmind-cli new -o out.xmind [-t 模板]          从模板新建 (.xmind)
  xmind-cli convert 输入 输出 [--scale 2]        任意支持的格式互转
  xmind-cli render 输入 -o out.png [--scale 2]   无头渲染 PNG/SVG/PDF
  xmind-cli info 输入                            查看文档统计
  xmind-cli templates                            列出内置模板

输入格式: .xmind .md .txt .opml .mm
输出格式: .xmind .md .txt .opml .mm .html .svg .png .pdf .docx .xlsx

示例:
  xmind-cli new -o plan.xmind -t project
  xmind-cli convert plan.xmind plan.md
  xmind-cli render plan.xmind -o plan.png --scale 2",
        env!("CARGO_PKG_VERSION")
    );
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(msg) => {
            if !msg.is_empty() {
                println!("{msg}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("错误: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: &[String]) -> Result<String, String> {
    let Some(cmd) = args.first() else {
        usage();
        return Ok(String::new());
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => {
            usage();
            Ok(String::new())
        }
        "templates" => {
            let mut s = String::from("内置模板:\n");
            for t in xmind_core::templates::TEMPLATES {
                s.push_str(&format!("  {:<12} {:<8} {}\n", t.id, t.name, t.desc));
            }
            Ok(s)
        }
        "new" => {
            let mut out: Option<PathBuf> = None;
            let mut template = "blank".to_string();
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "-o" | "--out" => {
                        out = args.get(i + 1).map(PathBuf::from);
                        i += 2;
                    }
                    "-t" | "--template" => {
                        template = args.get(i + 1).cloned().unwrap_or_else(|| "blank".into());
                        i += 2;
                    }
                    _ => i += 1,
                }
            }
            let out = out.ok_or("缺少 -o 输出路径")?;
            let doc = xmind_core::templates::make(&template);
            xmind_core::fmt::save_xmind(&doc, &out).map_err(|e| e.to_string())?;
            Ok(format!("已创建 {} ({})", out.display(), template))
        }
        "info" => {
            let path = args.get(1).map(Path::new).ok_or("缺少输入文件")?;
            let doc = xmind_core::export::import(path).map_err(|e| e.to_string())?;
            let mut s = format!("文件: {}\n主题数: {}\n", path.display(), {
                let mut n = 0;
                for sh in &doc.sheets {
                    n += sh.topic_count();
                }
                n
            });
            for (i, sh) in doc.sheets.iter().enumerate() {
                let marked = "*";
                let cur = if i == doc.current { marked } else { "" };
                s.push_str(&format!(
                    "  画布{}{}: “{}”  {} 个主题, {} 条联系线\n",
                    i + 1,
                    cur,
                    sh.title,
                    sh.topic_count(),
                    sh.relationships.len()
                ));
            }
            s.push_str(&format!("默认主题: {}\n", doc.theme));
            Ok(s)
        }
        "render" => {
            let mut input: Option<&String> = None;
            let mut out: Option<PathBuf> = None;
            let mut scale = 2.0f32;
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "-o" | "--out" => {
                        out = args.get(i + 1).map(PathBuf::from);
                        i += 2;
                    }
                    "--scale" => {
                        scale = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(2.0);
                        i += 2;
                    }
                    other => {
                        if !other.starts_with('-') {
                            input = args.get(i);
                        }
                        i += 1;
                    }
                }
            }
            let input = input.ok_or("缺少输入文件")?;
            let out = out.ok_or("缺少 -o 输出路径")?;
            let doc = xmind_core::export::import(Path::new(input)).map_err(|e| e.to_string())?;
            xmind_core::export::export(&doc, &out, scale).map_err(|e| e.to_string())?;
            let kb = std::fs::metadata(&out).map(|m| m.len() / 1024).unwrap_or(0);
            Ok(format!("已导出 {} ({} KB)", out.display(), kb))
        }
        "convert" => {
            let input = args.get(1).ok_or("缺少输入文件")?;
            let out = args.get(2).map(PathBuf::from).ok_or("缺少输出路径")?;
            let mut scale = 2.0f32;
            if let Some(p) = args.iter().position(|a| a == "--scale") {
                scale = args.get(p + 1).and_then(|s| s.parse().ok()).unwrap_or(2.0);
            }
            let doc = xmind_core::export::import(Path::new(input)).map_err(|e| e.to_string())?;
            xmind_core::export::export(&doc, &out, scale).map_err(|e| e.to_string())?;
            let kb = std::fs::metadata(&out).map(|m| m.len() / 1024).unwrap_or(0);
            Ok(format!("已转换 {} → {} ({} KB)", input, out.display(), kb))
        }
        other => Err(format!("未知命令: {other}（--help 查看用法）")),
    }
}
