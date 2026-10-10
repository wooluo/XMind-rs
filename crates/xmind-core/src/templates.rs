use crate::marker::{ColKey, Marker, Sym};
use crate::model::{Doc, Sheet, Structure, Topic};

pub struct TemplateDef {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
}

pub static TEMPLATES: &[TemplateDef] = &[
    TemplateDef { id: "blank", name: "空白图", desc: "从一个中心主题开始" },
    TemplateDef { id: "brainstorm", name: "头脑风暴", desc: "四个分支快速发散" },
    TemplateDef { id: "project", name: "项目计划", desc: "目标 / 阶段 / 风险" },
    TemplateDef { id: "reading", name: "读书笔记", desc: "书籍要点整理" },
    TemplateDef { id: "weekly", name: "一周安排", desc: "周一到周日" },
    TemplateDef { id: "meeting", name: "会议纪要", desc: "议题 / 结论 / 待办" },
    TemplateDef { id: "timeline", name: "时间线", desc: "项目里程碑" },
    TemplateDef { id: "fishbone", name: "鱼骨图", desc: "原因分析" },
];

pub fn make(id: &str) -> Doc {
    let mut doc = Doc::new();
    match id {
        "brainstorm" => {
            doc.theme = "vitality".into();
            let s = &mut doc.sheets[0];
            s.title = "头脑风暴".into();
            s.root.text = "中心问题".into();
            for (i, name) in ["想法", "问题", "资源", "行动"].iter().enumerate() {
                let mut t = Topic::new(*name);
                if i == 0 {
                    t.markers.push(Marker::Star(ColKey::Yellow));
                }
                for j in 1..=3 {
                    t.insert_child(None, Topic::new(format!("{name} {j}")));
                }
                s.root.insert_child(None, t);
            }
        }
        "project" => {
            doc.theme = "business".into();
            let s = &mut doc.sheets[0];
            s.title = "项目计划".into();
            s.root.text = "项目名称".into();
            let mut goal = Topic::new("目标");
            for g in ["核心指标", "交付物", "时间节点"] {
                goal.insert_child(None, Topic::new(g));
            }
            let mut phases = Topic::new("阶段");
            let tasks = [("启动", 100.0), ("设计", 50.0), ("开发", 12.5), ("测试", 0.0), ("上线", 0.0)];
            for (name, pct) in tasks {
                let mut t = Topic::new(name);
                t.markers.push(Marker::Task(pct));
                phases.insert_child(None, t);
            }
            let mut risk = Topic::new("风险");
            for r in ["进度风险", "技术风险", "人员风险"] {
                let mut t = Topic::new(r);
                t.markers.push(Marker::Symbol(Sym::Attention));
                risk.insert_child(None, t);
            }
            let mut team = Topic::new("团队");
            for p in ["产品", "研发", "设计"] {
                let mut t = Topic::new(p);
                t.markers.push(Marker::People(ColKey::Blue));
                team.insert_child(None, t);
            }
            for t in [goal, phases, risk, team] {
                s.root.insert_child(None, t);
            }
        }
        "reading" => {
            doc.theme = "fresh-green".into();
            let s = &mut doc.sheets[0];
            s.title = "读书笔记".into();
            s.root.text = "书名".into();
            for name in ["核心观点", "金句摘录", "章节结构", "行动启发", "评分"] {
                let mut t = Topic::new(name);
                if name == "评分" {
                    t.markers.push(Marker::Star(ColKey::Green));
                }
                t.insert_child(None, Topic::new("待补充"));
                s.root.insert_child(None, t);
            }
        }
        "weekly" => {
            doc.theme = "sakura".into();
            let s = &mut doc.sheets[0];
            s.title = "一周安排".into();
            s.root.text = "本周重点".into();
            s.root.insert_child(None, Topic::new("输入本周重点"));
            let s2 = Sheet::new("日程", "一周");
            let mut sh2 = s2;
            for d in ["周一", "周二", "周三", "周四", "周五", "周六", "周日"] {
                let mut t = Topic::new(d);
                t.insert_child(None, Topic::new("上午"));
                t.insert_child(None, Topic::new("下午"));
                sh2.root.insert_child(None, t);
            }
            doc.sheets.push(sh2);
        }
        "meeting" => {
            doc.theme = "classic-blue".into();
            let s = &mut doc.sheets[0];
            s.title = "会议纪要".into();
            s.root.text = "会议主题".into();
            let mut agenda = Topic::new("议题");
            agenda.labels.push("记录人：".into());
            for a in ["议题一", "议题二"] {
                agenda.insert_child(None, Topic::new(a));
            }
            let mut decisions = Topic::new("结论");
            decisions.markers.push(Marker::Priority(1));
            decisions.insert_child(None, Topic::new("结论一"));
            let mut actions = Topic::new("待办");
            for a in ["负责人", "截止时间"] {
                let mut t = Topic::new(a);
                t.markers.push(Marker::Task(0.0));
                actions.insert_child(None, t);
            }
            for t in [agenda, decisions, actions] {
                s.root.insert_child(None, t);
            }
        }
        "timeline" => {
            doc.theme = "violet".into();
            let s = &mut doc.sheets[0];
            s.title = "时间线".into();
            s.root.text = "项目里程碑".into();
            s.root.structure = Some(Structure::Timeline);
            for m in ["需求确认", "方案设计", "开发", "测试", "发布"] {
                s.root.insert_child(None, Topic::new(m));
            }
        }
        "fishbone" => {
            doc.theme = "dark-night".into();
            let s = &mut doc.sheets[0];
            s.title = "原因分析".into();
            s.root.text = "问题".into();
            s.root.structure = Some(Structure::Fishbone);
            for (i, cat) in ["人员", "流程", "设备", "环境"].iter().enumerate() {
                let mut t = Topic::new(*cat);
                if i % 2 == 0 {
                    t.markers.push(Marker::Symbol(Sym::Question));
                }
                for c in ["原因 1", "原因 2"] {
                    t.insert_child(None, Topic::new(c));
                }
                s.root.insert_child(None, t);
            }
        }
        _ => {
            doc.theme = "classic-blue".into();
        }
    }
    doc
}
